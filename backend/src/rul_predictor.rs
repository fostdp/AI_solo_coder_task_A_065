//! Remaining Useful Life (RUL) prediction module for CNC spindle bearings.
//!
//! Combines two complementary approaches:
//! - **SKF L10**: Physics-based bearing life calculation per ISO 281
//! - **LSTM**: Data-driven neural network for degradation trend extrapolation
//! - **Ensemble**: Weighted average (SKF 40%, LSTM 60%)
//!
//! Required `Cargo.toml` dependencies:
//! ```toml
//! [dependencies]
//! anyhow = "1"
//! ndarray = "0.16"
//! serde = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! ```

use anyhow::{bail, Context, Result};
use ndarray::{s, Array1, Array2};
use serde::{Deserialize, Serialize};
use std::fs;

// ─── Constants ───────────────────────────────────────────────────────────────

const SKF_C_KN: f64 = 69.5;
const BEARING_EXPONENT: f64 = 3.0;
const INPUT_SIZE: usize = 8;
const HIDDEN_SIZE: usize = 64;
const NUM_LAYERS: usize = 2;
const SEQ_LEN: usize = 48;
const SKF_WEIGHT: f64 = 0.4;
const LSTM_WEIGHT: f64 = 0.6;
const VIB_GREEN: f64 = 2.8;
const VIB_YELLOW: f64 = 7.1;
const RUL_MAINT_THRESHOLD: f64 = 500.0;
const RUL_URGENT_THRESHOLD: f64 = 200.0;
const NORMAL_TEMP: f64 = 40.0;
const MAX_TEMP: f64 = 80.0;
const MAX_VIB: f64 = 12.0;
const MAX_DISP: f64 = 50.0;
const RPM_MIN: f64 = 0.0;
const RPM_MAX: f64 = 24000.0;
const RUL_SMOOTH_ALPHA: f64 = 0.3;

// ─── Severity ────────────────────────────────────────────────────────────────

/// Vibration severity classification per ISO 10816.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// < 2.8 mm/s — acceptable
    Green,
    /// 2.8 – 7.1 mm/s — attention
    Yellow,
    /// > 7.1 mm/s — danger
    Red,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Green => write!(f, "Green"),
            Self::Yellow => write!(f, "Yellow"),
            Self::Red => write!(f, "Red"),
        }
    }
}

// ─── Operating Condition Classification ───────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatingCondition {
    Idle,
    LowSpeed,
    MediumSpeed,
    HighSpeed,
    Overload,
}

impl std::fmt::Display for OperatingCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::LowSpeed => write!(f, "LowSpeed"),
            Self::MediumSpeed => write!(f, "MediumSpeed"),
            Self::HighSpeed => write!(f, "HighSpeed"),
            Self::Overload => write!(f, "Overload"),
        }
    }
}

pub fn classify_operating_condition(rpm: f64, vibration_rms: f64) -> OperatingCondition {
    if rpm < 500.0 {
        OperatingCondition::Idle
    } else if vibration_rms > 10.0 {
        OperatingCondition::Overload
    } else if rpm < 8000.0 {
        OperatingCondition::LowSpeed
    } else if rpm < 16000.0 {
        OperatingCondition::MediumSpeed
    } else {
        OperatingCondition::HighSpeed
    }
}

// ─── Condition-specific Normalization ─────────────────────────────────────────

struct ConditionNormParams {
    vib_mean: f64,
    vib_std: f64,
    temp_mean: f64,
    temp_std: f64,
    disp_mean: f64,
    disp_std: f64,
    rpm_mean: f64,
    rpm_std: f64,
}

const NORM_PARAMS: &[ConditionNormParams; 5] = &[
    ConditionNormParams { vib_mean: 0.5, vib_std: 0.3, temp_mean: 35.0, temp_std: 2.0, disp_mean: 1.0, disp_std: 0.5, rpm_mean: 0.0, rpm_std: 100.0 },
    ConditionNormParams { vib_mean: 1.2, vib_std: 0.5, temp_mean: 38.0, temp_std: 2.5, disp_mean: 1.5, disp_std: 0.8, rpm_mean: 5000.0, rpm_std: 1500.0 },
    ConditionNormParams { vib_mean: 2.0, vib_std: 0.8, temp_mean: 40.0, temp_std: 3.0, disp_mean: 2.0, disp_std: 1.0, rpm_mean: 12000.0, rpm_std: 3000.0 },
    ConditionNormParams { vib_mean: 3.0, vib_std: 1.2, temp_mean: 43.0, temp_std: 3.5, disp_mean: 3.0, disp_std: 1.5, rpm_mean: 20000.0, rpm_std: 2500.0 },
    ConditionNormParams { vib_mean: 8.0, vib_std: 2.0, temp_mean: 50.0, temp_std: 5.0, disp_mean: 8.0, disp_std: 3.0, rpm_mean: 22000.0, rpm_std: 2000.0 },
];

fn get_norm_params(condition: OperatingCondition) -> &'static ConditionNormParams {
    match condition {
        OperatingCondition::Idle => &NORM_PARAMS[0],
        OperatingCondition::LowSpeed => &NORM_PARAMS[1],
        OperatingCondition::MediumSpeed => &NORM_PARAMS[2],
        OperatingCondition::HighSpeed => &NORM_PARAMS[3],
        OperatingCondition::Overload => &NORM_PARAMS[4],
    }
}

pub fn normalize_features(
    vibration_rms: f64,
    temperature: f64,
    temperature_rate: f64,
    displacement: f64,
    rpm: f64,
    health_score: f64,
    condition: OperatingCondition,
) -> Array1<f64> {
    let p = get_norm_params(condition);
    let vib_norm = (vibration_rms - p.vib_mean) / p.vib_std.max(0.01);
    let temp_norm = (temperature - p.temp_mean) / p.temp_std.max(0.01);
    let temp_rate_norm = temperature_rate / 1.0;
    let disp_norm = (displacement - p.disp_mean) / p.disp_std.max(0.01);
    let rpm_norm = (rpm - p.rpm_mean) / p.rpm_std.max(0.01);
    let load_factor = vibration_rms / VIB_YELLOW;
    let condition_code = match condition {
        OperatingCondition::Idle => 0.0,
        OperatingCondition::LowSpeed => 0.25,
        OperatingCondition::MediumSpeed => 0.5,
        OperatingCondition::HighSpeed => 0.75,
        OperatingCondition::Overload => 1.0,
    };
    Array1::from_vec(vec![
        vib_norm,
        temp_norm,
        temp_rate_norm,
        disp_norm,
        rpm_norm,
        load_factor,
        condition_code,
        health_score / 100.0,
    ])
}

// ─── Health Score ────────────────────────────────────────────────────────────

/// Composite health score (0–100) with severity classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    pub score: f32,
    pub vibration_severity: Severity,
    pub temperature_status: String,
}

/// Classify vibration RMS into ISO 10816 severity zones.
pub fn classify_vibration_severity(vibration_rms: f64) -> Severity {
    if vibration_rms < VIB_GREEN {
        Severity::Green
    } else if vibration_rms <= VIB_YELLOW {
        Severity::Yellow
    } else {
        Severity::Red
    }
}

/// Return a human-readable temperature status label.
pub fn calculate_temperature_status(temperature: f64) -> String {
    if temperature <= NORMAL_TEMP {
        "Normal".into()
    } else if temperature <= 60.0 {
        "Elevated".into()
    } else if temperature <= MAX_TEMP {
        "High".into()
    } else {
        "Critical".into()
    }
}

/// Compute composite health score from vibration, temperature, and displacement.
///
/// Weighting: vibration 50 %, temperature 30 %, displacement 20 %.
pub fn calculate_health_score(vibration_rms: f64, temperature: f64, displacement: f64) -> HealthScore {
    let vib_score = ((1.0 - (vibration_rms / MAX_VIB).min(1.0)) * 50.0) as f32;
    let temp_score =
        ((1.0 - ((temperature - NORMAL_TEMP) / (MAX_TEMP - NORMAL_TEMP)).clamp(0.0, 1.0)) * 30.0) as f32;
    let disp_score = ((1.0 - (displacement / MAX_DISP).min(1.0)) * 20.0) as f32;
    let score = (vib_score + temp_score + disp_score).clamp(0.0, 100.0);
    HealthScore {
        score,
        vibration_severity: classify_vibration_severity(vibration_rms),
        temperature_status: calculate_temperature_status(temperature),
    }
}

// ─── Temperature rate of change ──────────────────────────────────────────────

/// Estimate the rate of temperature change (°C / interval) via linear regression.
pub fn calculate_temperature_rate(temperatures: &[f64], interval_hours: f64) -> f64 {
    if temperatures.len() < 2 || interval_hours <= 0.0 {
        return 0.0;
    }
    let (slope, _) = linear_regression(temperatures);
    slope * interval_hours
}

// ─── Degradation trend ───────────────────────────────────────────────────────

/// Detect degradation trend via least-squares linear regression on vibration RMS.
///
/// Returns `(slope, intercept)`. Positive slope indicates worsening condition.
pub fn detect_degradation_trend(vibration_history: &[f64]) -> (f64, f64) {
    linear_regression(vibration_history)
}

/// Ordinary least-squares linear regression y = slope * x + intercept.
fn linear_regression(data: &[f64]) -> (f64, f64) {
    let n = data.len() as f64;
    if n < 2.0 {
        return (0.0, data.first().copied().unwrap_or(0.0));
    }
    let x_mean = (n - 1.0) / 2.0;
    let y_mean = data.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, &y) in data.iter().enumerate() {
        let xi = i as f64 - x_mean;
        num += xi * (y - y_mean);
        den += xi * xi;
    }
    if den.abs() < 1e-12 {
        return (0.0, y_mean);
    }
    let slope = num / den;
    let intercept = y_mean - slope * x_mean;
    (slope, intercept)
}

// ─── SKF L10 Bearing Life ───────────────────────────────────────────────────

/// Calculate adjusted SKF L10aa remaining useful life (hours).
///
/// * `rpm` — rotational speed
/// * `vibration_rms` — current vibration RMS (mm/s), used to estimate load increase
/// * `temperature` — current bearing temperature (°C)
/// * `baseline_load_kn` — nominal equivalent dynamic load (kN)
/// * `contamination_level` — 0.0 (clean) to 1.0 (heavily contaminated)
pub fn calculate_skf_rul(
    rpm: f64,
    vibration_rms: f64,
    temperature: f64,
    baseline_load_kn: f64,
    contamination_level: f64,
) -> f64 {
    if rpm <= 0.0 {
        return 0.0;
    }
    let load_factor = 1.0 + (vibration_rms / VIB_YELLOW).min(3.0) * 0.5;
    let p = baseline_load_kn * load_factor;

    let l10 = (SKF_C_KN / p).powf(BEARING_EXPONENT) * (1_000_000.0 / (60.0 * rpm));

    let a1 = 0.33; // 99 % reliability
    let a2 = 1.0; // standard steel
    let temp_factor = if temperature > 70.0 {
        0.5
    } else if temperature > 50.0 {
        0.75
    } else {
        1.0
    };
    let cont_factor = (1.0 - contamination_level * 0.5).max(0.3);
    let a3 = temp_factor * cont_factor;

    (a1 * a2 * a3 * l10).max(0.0)
}

// ─── LSTM Neural Network ────────────────────────────────────────────────────

/// Weights for a single LSTM layer (combined gate matrices).
///
/// Gate ordering in the `4 × H` dimension: input, forget, cell, output.
#[derive(Debug, Clone)]
pub struct LSTMLayerWeights {
    pub w_i: Array2<f64>,
    pub w_h: Array2<f64>,
    pub b_i: Array1<f64>,
    pub b_h: Array1<f64>,
}

/// Full LSTM model weights (2 layers + dense output).
#[derive(Debug, Clone)]
pub struct LSTMWeights {
    pub layers: Vec<LSTMLayerWeights>,
    pub dense_w: Array2<f64>,
    pub dense_b: Array1<f64>,
}

// ─── JSON deserialization helpers ─────────────────────────────────────────────

#[derive(Deserialize)]
struct LayerJson {
    w_i: Vec<Vec<f64>>,
    w_h: Vec<Vec<f64>>,
    b_i: Vec<f64>,
    b_h: Vec<f64>,
}

#[derive(Deserialize)]
struct WeightsJson {
    layers: Vec<LayerJson>,
    dense_w: Vec<Vec<f64>>,
    dense_b: Vec<f64>,
}

fn vec2d_to_array2(data: &[Vec<f64>]) -> Result<Array2<f64>> {
    if data.is_empty() {
        bail!("empty weight matrix");
    }
    let rows = data.len();
    let cols = data[0].len();
    for row in data {
        if row.len() != cols {
            bail!("inconsistent row lengths in weight matrix");
        }
    }
    let flat: Vec<f64> = data.iter().flatten().copied().collect();
    Ok(Array2::from_shape_vec((rows, cols), flat)?)
}

impl WeightsJson {
    fn into_weights(self) -> Result<LSTMWeights> {
        let mut layers = Vec::with_capacity(self.layers.len());
        for l in self.layers {
            layers.push(LSTMLayerWeights {
                w_i: vec2d_to_array2(&l.w_i).context("layer w_i")?,
                w_h: vec2d_to_array2(&l.w_h).context("layer w_h")?,
                b_i: Array1::from_vec(l.b_i),
                b_h: Array1::from_vec(l.b_h),
            });
        }
        Ok(LSTMWeights {
            layers,
            dense_w: vec2d_to_array2(&self.dense_w).context("dense_w")?,
            dense_b: Array1::from_vec(self.dense_b),
        })
    }
}

// ─── Activation functions ────────────────────────────────────────────────────

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn softplus(x: f64) -> f64 {
    if x > 20.0 { x } else { (1.0 + x.exp()).ln() }
}

// ─── Simple deterministic PRNG (avoids `rand` dependency) ────────────────────

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: if seed == 0 { 1 } else { seed } }
    }

    fn next_f64(&mut self) -> f64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.state >> 33) as f64 / (1u64 << 31) as f64
    }

    fn gen_range(&mut self, low: f64, high: f64) -> f64 {
        low + self.next_f64() * (high - low)
    }
}

// ─── LSTMWeights impl ───────────────────────────────────────────────────────

impl LSTMWeights {
    /// Load pre-trained weights from a JSON file.
    pub fn load_from_json(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path).context("failed to read weights file")?;
        let json: WeightsJson =
            serde_json::from_str(&content).context("failed to parse weights JSON")?;
        json.into_weights()
    }

    /// Generate Xavier-initialized weights for demo / testing.
    pub fn generate_demo() -> Self {
        let mut rng = SimpleRng::new(42);

        let make_array2 = |rows: usize, cols: usize, fan: (usize, usize), rng: &mut SimpleRng| {
            let limit = (6.0 / (fan.0 + fan.1) as f64).sqrt();
            let flat: Vec<f64> = (0..rows * cols).map(|_| rng.gen_range(-limit, limit)).collect();
            Array2::from_shape_vec((rows, cols), flat).unwrap()
        };

        let mut layers = Vec::with_capacity(NUM_LAYERS);
        for layer_idx in 0..NUM_LAYERS {
            let in_size = if layer_idx == 0 { INPUT_SIZE } else { HIDDEN_SIZE };
            let gate_size = 4 * HIDDEN_SIZE;
            layers.push(LSTMLayerWeights {
                w_i: make_array2(gate_size, in_size, (in_size, HIDDEN_SIZE), &mut rng),
                w_h: make_array2(gate_size, HIDDEN_SIZE, (HIDDEN_SIZE, HIDDEN_SIZE), &mut rng),
                b_i: Array1::zeros(gate_size),
                b_h: Array1::zeros(gate_size),
            });
        }

        let dense_limit = (6.0 / (HIDDEN_SIZE + 1) as f64).sqrt();
        let dense_flat: Vec<f64> = (0..HIDDEN_SIZE)
            .map(|_| rng.gen_range(-dense_limit, dense_limit))
            .collect();

        LSTMWeights {
            layers,
            dense_w: Array2::from_shape_vec((1, HIDDEN_SIZE), dense_flat).unwrap(),
            dense_b: Array1::zeros(1),
        }
    }
}

// ─── LSTM cell & forward pass ────────────────────────────────────────────────

/// Single LSTM cell forward step.
///
/// Gate ordering: input (i), forget (f), cell (g), output (o).
fn lstm_cell_forward(
    x: &Array1<f64>,
    h_prev: &Array1<f64>,
    c_prev: &Array1<f64>,
    weights: &LSTMLayerWeights,
) -> (Array1<f64>, Array1<f64>) {
    let gates = weights.w_i.dot(x) + &weights.b_i + weights.w_h.dot(h_prev) + &weights.b_h;

    let h = HIDDEN_SIZE;
    let i = gates.slice(s![0..h]).mapv(sigmoid);
    let f = gates.slice(s![h..2 * h]).mapv(sigmoid);
    let g = gates.slice(s![2 * h..3 * h]).mapv(|v| v.tanh());
    let o = gates.slice(s![3 * h..4 * h]).mapv(sigmoid);

    let c_new = &f * c_prev + &i * &g;
    let h_new = &o * c_new.mapv(|v| v.tanh());

    (h_new, c_new)
}

/// Full LSTM forward pass through all layers and time steps, producing an RUL estimate.
pub fn lstm_forward(weights: &LSTMWeights, input_sequence: &[Array1<f64>]) -> f64 {
    let mut layer_states: Vec<(Array1<f64>, Array1<f64>)> = weights
        .layers
        .iter()
        .map(|_| (Array1::zeros(HIDDEN_SIZE), Array1::zeros(HIDDEN_SIZE)))
        .collect();

    for t in 0..input_sequence.len() {
        let mut layer_input = input_sequence[t].clone();
        for (layer_idx, layer_weights) in weights.layers.iter().enumerate() {
            let (ref h, ref c) = layer_states[layer_idx];
            let (h_new, c_new) = lstm_cell_forward(&layer_input, h, c, layer_weights);
            layer_states[layer_idx] = (h_new.clone(), c_new);
            layer_input = h_new;
        }
    }

    let (final_h, _) = layer_states.last().expect("at least one LSTM layer");
    let raw = weights.dense_w.dot(final_h) + &weights.dense_b;
    softplus(raw[0])
}

// ─── RUL Result ──────────────────────────────────────────────────────────────

/// Ensemble RUL prediction result combining SKF and LSTM outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RULResult {
    pub rul_hours: f64,
    pub skf_rul: f64,
    pub lstm_rul: f64,
    pub confidence: f64,
    pub degradation_rate: f64,
}

// ─── Maintenance Order ───────────────────────────────────────────────────────

/// Auto-generated maintenance order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceOrder {
    pub machine_id: u16,
    pub order_type: String,
    pub priority: String,
    pub description: String,
    pub rul_at_creation: f64,
}

// ─── RUL Predictor ──────────────────────────────────────────────────────────

/// Top-level predictor that combines SKF physics-based and LSTM data-driven models.
#[derive(Debug, Clone)]
pub struct RULPredictor {
    pub lstm_weights: LSTMWeights,
    pub clickhouse_url: String,
    rul_cache: std::cell::RefCell<HashMap<u16, f64>>,
    condition_cache: std::cell::RefCell<HashMap<u16, OperatingCondition>>,
}

impl RULPredictor {
    /// Create a new predictor with demo (Xavier-initialized) LSTM weights.
    pub fn new(clickhouse_url: &str) -> Self {
        Self {
            lstm_weights: LSTMWeights::generate_demo(),
            clickhouse_url: clickhouse_url.to_string(),
            rul_cache: std::cell::RefCell::new(HashMap::new()),
            condition_cache: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn with_weights(clickhouse_url: &str, weights_path: &str) -> Result<Self> {
        let lstm_weights = LSTMWeights::load_from_json(weights_path)?;
        Ok(Self {
            lstm_weights,
            clickhouse_url: clickhouse_url.to_string(),
            rul_cache: std::cell::RefCell::new(HashMap::new()),
            condition_cache: std::cell::RefCell::new(HashMap::new()),
        })
    }

    /// Run the full ensemble RUL prediction pipeline.
    ///
    /// * `rpm` — current spindle speed
    /// * `vibration_rms` — current vibration RMS (mm/s)
    /// * `temperature` — current bearing temperature (°C)
    /// * `displacement` — current axial displacement (μm)
    /// * `baseline_load_kn` — nominal equivalent dynamic load (kN)
    /// * `contamination_level` — 0.0–1.0 contamination severity
    /// * `vibration_history` — recent vibration RMS readings for trend detection
    /// * `feature_window` — sliding window of the last 48 hourly feature vectors
    ///   (each vector: `[vibration_rms, temperature, temperature_rate, displacement, rpm, health_score]`)
    pub fn predict_rul(
        &self,
        rpm: f64,
        vibration_rms: f64,
        temperature: f64,
        displacement: f64,
        baseline_load_kn: f64,
        contamination_level: f64,
        vibration_history: &[f64],
        feature_window: &[Array1<f64>],
    ) -> RULResult {
        let current_condition = classify_operating_condition(rpm, vibration_rms);

        let skf_rul = calculate_skf_rul(
            rpm,
            vibration_rms,
            temperature,
            baseline_load_kn,
            contamination_level,
        );

        let normalized_window: Vec<Array1<f64>> = if feature_window.len() >= SEQ_LEN {
            feature_window.iter().map(|f| {
                let vr = f.get(0).copied().unwrap_or(0.0);
                let t = f.get(1).copied().unwrap_or(40.0);
                let tr = f.get(2).copied().unwrap_or(0.0);
                let d = f.get(3).copied().unwrap_or(0.0);
                let r = f.get(4).copied().unwrap_or(rpm);
                let hs = f.get(5).copied().unwrap_or(80.0);
                normalize_features(vr, t, tr, d, r, hs, current_condition)
            }).collect()
        } else {
            let health_score = calculate_health_score(vibration_rms, temperature, displacement).score;
            let normalized = normalize_features(
                vibration_rms, temperature, 0.05, displacement, rpm,
                health_score as f64, current_condition,
            );
            vec![normalized; SEQ_LEN]
        };

        let lstm_rul = if normalized_window.len() >= SEQ_LEN {
            lstm_forward(&self.lstm_weights, &normalized_window)
        } else {
            skf_rul
        };

        let condition_baseline = match current_condition {
            OperatingCondition::Idle => 8000.0,
            OperatingCondition::LowSpeed => 5000.0,
            OperatingCondition::MediumSpeed => 3000.0,
            OperatingCondition::HighSpeed => 1500.0,
            OperatingCondition::Overload => 500.0,
        };

        let condition_weight = match current_condition {
            OperatingCondition::Idle => 0.0,
            OperatingCondition::LowSpeed => 0.1,
            OperatingCondition::MediumSpeed => 0.15,
            OperatingCondition::HighSpeed => 0.2,
            OperatingCondition::Overload => 0.3,
        };
        let raw_rul = (1.0 - condition_weight) * (SKF_WEIGHT * skf_rul + LSTM_WEIGHT * lstm_rul)
            + condition_weight * condition_baseline.min(skf_rul + lstm_rul) * 0.5;

        let smoothed_rul = {
            let cache = self.rul_cache.borrow();
            if let Some(&prev_rul) = cache.get(&0u16) {
                RUL_SMOOTH_ALPHA * raw_rul + (1.0 - RUL_SMOOTH_ALPHA) * prev_rul
            } else {
                raw_rul
            }
        };
        self.rul_cache.borrow_mut().insert(0u16, smoothed_rul);
        self.condition_cache.borrow_mut().insert(0u16, current_condition);

        let (slope, _) = detect_degradation_trend(vibration_history);
        let degradation_rate = slope.max(0.0);

        let data_factor = (feature_window.len() as f64 / SEQ_LEN as f64).min(1.0);
        let agreement = 1.0
            - ((skf_rul - lstm_rul).abs() / skf_rul.max(lstm_rul).max(1.0)).min(1.0);
        let condition_stability = {
            let cond_cache = self.condition_cache.borrow();
            if cond_cache.len() > 1 { 0.9 } else { 0.7 }
        };
        let confidence = (data_factor * 0.4 + agreement * 0.3 + condition_stability * 0.3).clamp(0.0, 1.0);

        RULResult {
            rul_hours: smoothed_rul.max(0.0),
            skf_rul: skf_rul.max(0.0),
            lstm_rul: lstm_rul.max(0.0),
            confidence,
            degradation_rate,
        }
    }

    pub fn predict_rul_for_machine(
        &self,
        machine_id: u16,
        rpm: f64,
        vibration_rms: f64,
        temperature: f64,
        displacement: f64,
        baseline_load_kn: f64,
        contamination_level: f64,
        vibration_history: &[f64],
        feature_window: &[Array1<f64>],
    ) -> RULResult {
        let current_condition = classify_operating_condition(rpm, vibration_rms);

        let skf_rul = calculate_skf_rul(
            rpm, vibration_rms, temperature,
            baseline_load_kn, contamination_level,
        );

        let health_score = calculate_health_score(vibration_rms, temperature, displacement).score;
        let normalized = normalize_features(
            vibration_rms, temperature, 0.05, displacement, rpm,
            health_score as f64, current_condition,
        );

        let normalized_window: Vec<Array1<f64>> = if feature_window.len() >= SEQ_LEN {
            feature_window.iter().map(|f| {
                let vr = f.get(0).copied().unwrap_or(0.0);
                let t = f.get(1).copied().unwrap_or(40.0);
                let tr = f.get(2).copied().unwrap_or(0.0);
                let d = f.get(3).copied().unwrap_or(0.0);
                let r = f.get(4).copied().unwrap_or(rpm);
                let hs = f.get(5).copied().unwrap_or(80.0);
                normalize_features(vr, t, tr, d, r, hs, current_condition)
            }).collect()
        } else {
            vec![normalized; SEQ_LEN]
        };

        let lstm_rul = if normalized_window.len() >= SEQ_LEN {
            lstm_forward(&self.lstm_weights, &normalized_window)
        } else {
            skf_rul
        };

        let condition_baseline = match current_condition {
            OperatingCondition::Idle => 8000.0,
            OperatingCondition::LowSpeed => 5000.0,
            OperatingCondition::MediumSpeed => 3000.0,
            OperatingCondition::HighSpeed => 1500.0,
            OperatingCondition::Overload => 500.0,
        };

        let condition_weight = match current_condition {
            OperatingCondition::Idle => 0.0,
            OperatingCondition::LowSpeed => 0.1,
            OperatingCondition::MediumSpeed => 0.15,
            OperatingCondition::HighSpeed => 0.2,
            OperatingCondition::Overload => 0.3,
        };
        let raw_rul = (1.0 - condition_weight) * (SKF_WEIGHT * skf_rul + LSTM_WEIGHT * lstm_rul)
            + condition_weight * condition_baseline.min(skf_rul + lstm_rul) * 0.5;

        let smoothed_rul = {
            let cache = self.rul_cache.borrow();
            if let Some(&prev_rul) = cache.get(&machine_id) {
                RUL_SMOOTH_ALPHA * raw_rul + (1.0 - RUL_SMOOTH_ALPHA) * prev_rul
            } else {
                raw_rul
            }
        };
        self.rul_cache.borrow_mut().insert(machine_id, smoothed_rul);
        self.condition_cache.borrow_mut().insert(machine_id, current_condition);

        let (slope, _) = detect_degradation_trend(vibration_history);
        let degradation_rate = slope.max(0.0);

        let data_factor = (feature_window.len() as f64 / SEQ_LEN as f64).min(1.0);
        let agreement = 1.0
            - ((skf_rul - lstm_rul).abs() / skf_rul.max(lstm_rul).max(1.0)).min(1.0);
        let confidence = (data_factor * 0.5 + agreement * 0.5).clamp(0.0, 1.0);

        RULResult {
            rul_hours: smoothed_rul.max(0.0),
            skf_rul: skf_rul.max(0.0),
            lstm_rul: lstm_rul.max(0.0),
            confidence,
            degradation_rate,
        }
    }

    /// Generate maintenance orders based on the predicted RUL.
    ///
    /// * RUL < 500 h → tool-change suggestion (priority: high)
    /// * RUL < 200 h → urgent bearing replacement (priority: urgent)
    pub fn generate_maintenance_orders(
        &self,
        machine_id: u16,
        rul: &RULResult,
    ) -> Vec<MaintenanceOrder> {
        let mut orders = Vec::new();
        if rul.rul_hours < RUL_MAINT_THRESHOLD {
            let (priority, order_type, desc) = if rul.rul_hours < RUL_URGENT_THRESHOLD {
                (
                    "urgent",
                    "urgent_bearing_replacement",
                    format!(
                        "URGENT: Bearing RUL critically low at {:.0}h. \
                         Immediate replacement required. Degradation rate: {:.3} mm/s per hour.",
                        rul.rul_hours, rul.degradation_rate
                    ),
                )
            } else {
                (
                    "high",
                    "tool_change_suggestion",
                    format!(
                        "Scheduled maintenance recommended: Bearing RUL at {:.0}h. \
                         Plan tool change within next maintenance window. \
                         Degradation rate: {:.3} mm/s per hour.",
                        rul.rul_hours, rul.degradation_rate
                    ),
                )
            };
            orders.push(MaintenanceOrder {
                machine_id,
                order_type: order_type.to_string(),
                priority: priority.to_string(),
                description: desc,
                rul_at_creation: rul.rul_hours,
            });
        }
        orders
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vibration_severity() {
        assert_eq!(classify_vibration_severity(1.0), Severity::Green);
        assert_eq!(classify_vibration_severity(4.0), Severity::Yellow);
        assert_eq!(classify_vibration_severity(8.0), Severity::Red);
        assert_eq!(classify_vibration_severity(VIB_GREEN), Severity::Green);
        assert_eq!(classify_vibration_severity(VIB_YELLOW), Severity::Yellow);
    }

    #[test]
    fn test_health_score() {
        let hs = calculate_health_score(1.0, 35.0, 5.0);
        assert!(hs.score > 70.0);
        assert_eq!(hs.vibration_severity, Severity::Green);
        assert_eq!(hs.temperature_status, "Normal");

        let hs_bad = calculate_health_score(10.0, 90.0, 40.0);
        assert!(hs_bad.score < 30.0);
        assert_eq!(hs_bad.vibration_severity, Severity::Red);
    }

    #[test]
    fn test_skf_rul() {
        let rul = calculate_skf_rul(3000.0, 2.0, 40.0, 5.0, 0.1);
        assert!(rul > 0.0);
        let rul_zero_rpm = calculate_skf_rul(0.0, 2.0, 40.0, 5.0, 0.1);
        assert_eq!(rul_zero_rpm, 0.0);
    }

    #[test]
    fn test_degradation_trend() {
        let history: Vec<f64> = (0..48).map(|i| 1.0 + i as f64 * 0.1).collect();
        let (slope, intercept) = detect_degradation_trend(&history);
        assert!(slope > 0.0);
        assert!(intercept > 0.0);
    }

    #[test]
    fn test_temperature_rate() {
        let temps = vec![40.0, 42.0, 44.0, 46.0, 48.0];
        let rate = calculate_temperature_rate(&temps, 1.0);
        assert!(rate > 0.0);
    }

    #[test]
    fn test_lstm_demo_weights() {
        let weights = LSTMWeights::generate_demo();
        assert_eq!(weights.layers.len(), NUM_LAYERS);
        assert_eq!(weights.layers[0].w_i.nrows(), 4 * HIDDEN_SIZE);
        assert_eq!(weights.layers[0].w_i.ncols(), INPUT_SIZE);
        assert_eq!(weights.layers[1].w_i.ncols(), HIDDEN_SIZE);
        assert_eq!(weights.dense_w.nrows(), 1);
        assert_eq!(weights.dense_w.ncols(), HIDDEN_SIZE);
    }

    #[test]
    fn test_lstm_forward() {
        let weights = LSTMWeights::generate_demo();
        let input: Vec<Array1<f64>> = (0..SEQ_LEN)
            .map(|_| Array1::from_vec(vec![2.5, 45.0, 0.1, 10.0, 3000.0, 0.35, 0.5, 0.75]))
            .collect();
        let output = lstm_forward(&weights, &input);
        assert!(output >= 0.0, "softplus output must be non-negative");
    }

    #[test]
    fn test_ensemble_rul() {
        let predictor = RULPredictor::new("http://localhost:8123");
        let history: Vec<f64> = (0..48).map(|i| 1.0 + i as f64 * 0.1).collect();
        let features: Vec<Array1<f64>> = (0..SEQ_LEN)
            .map(|i| Array1::from_vec(vec![history[i], 45.0, 0.05, 10.0, 3000.0, 0.35, 0.5, 80.0]))
            .collect();
        let result = predictor.predict_rul(
            3000.0, 5.0, 45.0, 10.0, 5.0, 0.1, &history, &features,
        );
        assert!(result.rul_hours >= 0.0);
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
        assert!(result.degradation_rate >= 0.0);
    }

    #[test]
    fn test_maintenance_orders_urgent() {
        let predictor = RULPredictor::new("http://localhost:8123");
        let urgent = RULResult {
            rul_hours: 100.0,
            skf_rul: 120.0,
            lstm_rul: 90.0,
            confidence: 0.8,
            degradation_rate: 0.5,
        };
        let orders = predictor.generate_maintenance_orders(1, &urgent);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].priority, "urgent");
        assert_eq!(orders[0].order_type, "urgent_bearing_replacement");
    }

    #[test]
    fn test_maintenance_orders_high() {
        let predictor = RULPredictor::new("http://localhost:8123");
        let high = RULResult {
            rul_hours: 350.0,
            skf_rul: 400.0,
            lstm_rul: 320.0,
            confidence: 0.7,
            degradation_rate: 0.2,
        };
        let orders = predictor.generate_maintenance_orders(1, &high);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].priority, "high");
        assert_eq!(orders[0].order_type, "tool_change_suggestion");
    }

    #[test]
    fn test_maintenance_orders_none() {
        let predictor = RULPredictor::new("http://localhost:8123");
        let normal = RULResult {
            rul_hours: 1000.0,
            skf_rul: 1100.0,
            lstm_rul: 900.0,
            confidence: 0.9,
            degradation_rate: 0.1,
        };
        let orders = predictor.generate_maintenance_orders(1, &normal);
        assert!(orders.is_empty());
    }
}
