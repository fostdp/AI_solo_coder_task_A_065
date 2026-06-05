use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use crate::models::*;
use crate::clickhouse_client::ClickHouseClient;
use log::{info, debug};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatingCondition {
    Idle,
    LowSpeedLowLoad,
    MediumSpeedMediumLoad,
    HighSpeedHighLoad,
    Cutting,
}

#[derive(Debug, Clone)]
pub struct ConditionNormalizationParams {
    pub vibration_rms_mean: f32,
    pub vibration_rms_std: f32,
    pub temperature_mean: f32,
    pub temperature_std: f32,
}

impl Default for ConditionNormalizationParams {
    fn default() -> Self {
        ConditionNormalizationParams {
            vibration_rms_mean: 1.5,
            vibration_rms_std: 0.8,
            temperature_mean: 45.0,
            temperature_std: 8.0,
        }
    }
}

struct BearingParams {
    rated_life: f32,
    dynamic_load_rating: f32,
    equivalent_load: f32,
    life_exponent: f32,
}

impl Default for BearingParams {
    fn default() -> Self {
        BearingParams {
            rated_life: 10000.0,
            dynamic_load_rating: 50000.0,
            equivalent_load: 5000.0,
            life_exponent: 3.0,
        }
    }
}

struct MachinePredictionState {
    last_rul: f32,
    last_condition: OperatingCondition,
    condition_history: Vec<OperatingCondition>,
    rul_smoothing_window: Vec<f32>,
    feature_history: Vec<NormalizedFeatures>,
}

impl Default for MachinePredictionState {
    fn default() -> Self {
        MachinePredictionState {
            last_rul: 8000.0,
            last_condition: OperatingCondition::Idle,
            condition_history: Vec::with_capacity(100),
            rul_smoothing_window: Vec::with_capacity(10),
            feature_history: Vec::with_capacity(100),
        }
    }
}

#[derive(Debug, Clone)]
struct NormalizedFeatures {
    vibration_rms_norm: f32,
    temperature_norm: f32,
    speed_norm: f32,
    load_norm: f32,
    condition: OperatingCondition,
}

pub struct PredictionEngine {
    clickhouse: ClickHouseClient,
    bearing_params: BearingParams,
    condition_params: HashMap<OperatingCondition, ConditionNormalizationParams>,
    machine_states: Arc<Mutex<HashMap<u16, MachinePredictionState>>>,
}

impl PredictionEngine {
    pub fn new(clickhouse: ClickHouseClient) -> Self {
        let mut condition_params = HashMap::new();

        condition_params.insert(OperatingCondition::Idle, ConditionNormalizationParams {
            vibration_rms_mean: 0.5,
            vibration_rms_std: 0.2,
            temperature_mean: 25.0,
            temperature_std: 3.0,
        });

        condition_params.insert(OperatingCondition::LowSpeedLowLoad, ConditionNormalizationParams {
            vibration_rms_mean: 1.0,
            vibration_rms_std: 0.5,
            temperature_mean: 35.0,
            temperature_std: 5.0,
        });

        condition_params.insert(OperatingCondition::MediumSpeedMediumLoad, ConditionNormalizationParams {
            vibration_rms_mean: 1.8,
            vibration_rms_std: 0.9,
            temperature_mean: 45.0,
            temperature_std: 8.0,
        });

        condition_params.insert(OperatingCondition::HighSpeedHighLoad, ConditionNormalizationParams {
            vibration_rms_mean: 2.5,
            vibration_rms_std: 1.2,
            temperature_mean: 55.0,
            temperature_std: 10.0,
        });

        condition_params.insert(OperatingCondition::Cutting, ConditionNormalizationParams {
            vibration_rms_mean: 3.0,
            vibration_rms_std: 1.5,
            temperature_mean: 60.0,
            temperature_std: 12.0,
        });

        PredictionEngine {
            clickhouse,
            bearing_params: BearingParams::default(),
            condition_params,
            machine_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn classify_operating_condition(spindle_speed: f32, load: f32) -> OperatingCondition {
        if spindle_speed < 100.0 && load < 5.0 {
            OperatingCondition::Idle
        } else if spindle_speed < 3000.0 && load < 30.0 {
            OperatingCondition::LowSpeedLowLoad
        } else if spindle_speed < 8000.0 && load < 60.0 {
            OperatingCondition::MediumSpeedMediumLoad
        } else if load > 70.0 {
            OperatingCondition::Cutting
        } else {
            OperatingCondition::HighSpeedHighLoad
        }
    }

    fn normalize_feature(value: f32, mean: f32, std: f32) -> f32 {
        if std < 1e-6 {
            return 0.0;
        }
        ((value - mean) / std).clamp(-3.0, 3.0)
    }

    fn get_normalization_params(&self, condition: OperatingCondition) -> ConditionNormalizationParams {
        self.condition_params.get(&condition)
            .copied()
            .unwrap_or_default()
    }

    pub async fn predict_rul(
        &self,
        machine_id: u16,
        current_data: &[&SensorData],
    ) -> anyhow::Result<Option<RULPrediction>> {
        let vibration_data: Vec<f32> = current_data
            .iter()
            .filter(|d| d.sensor_type == SensorType::Vibration)
            .map(|d| d.value.abs())
            .collect();

        let temperature_data: Vec<f32> = current_data
            .iter()
            .filter(|d| d.sensor_type == SensorType::Temperature)
            .map(|d| d.value)
            .collect();

        let spindle_speeds: Vec<f32> = current_data
            .iter()
            .map(|d| d.spindle_speed)
            .collect();

        let loads: Vec<f32> = current_data
            .iter()
            .map(|d| d.load)
            .collect();

        if vibration_data.is_empty() || temperature_data.is_empty() {
            return Ok(None);
        }

        let avg_speed = spindle_speeds.iter().sum::<f32>() / spindle_speeds.len() as f32;
        let avg_load = loads.iter().sum::<f32>() / loads.len() as f32;
        let current_condition = Self::classify_operating_condition(avg_speed, avg_load);

        let norm_params = self.get_normalization_params(current_condition);

        let vibration_rms = calculate_rms(&vibration_data);
        let avg_temp = temperature_data.iter().sum::<f32>() / temperature_data.len() as f32;

        let vibration_rms_norm = Self::normalize_feature(
            vibration_rms,
            norm_params.vibration_rms_mean,
            norm_params.vibration_rms_std,
        );

        let temperature_norm = Self::normalize_feature(
            avg_temp,
            norm_params.temperature_mean,
            norm_params.temperature_std,
        );

        let speed_norm = Self::normalize_feature(avg_speed, 5000.0, 3000.0);
        let load_norm = Self::normalize_feature(avg_load, 50.0, 25.0);

        let features = NormalizedFeatures {
            vibration_rms_norm,
            temperature_norm,
            speed_norm,
            load_norm,
            condition: current_condition,
        };

        let vibration_rms_trend = self.calculate_vibration_trend(machine_id, vibration_rms_norm).await;
        let temperature_rate = self.calculate_temperature_rate(machine_id, temperature_norm).await;

        let skf_l10_life = self.calculate_skf_l10(vibration_rms_norm, temperature_norm, load_norm, current_condition);
        let lstm_prediction = self.simulate_lstm_prediction(
            machine_id,
            vibration_rms_norm,
            temperature_norm,
            speed_norm,
            load_norm,
            current_condition,
            skf_l10_life,
        ).await;

        let raw_rul = (skf_l10_life * 0.4 + lstm_prediction * 0.6).max(0.0);

        let smoothed_rul = self.smooth_rul(machine_id, raw_rul).await;

        let rul_confidence = self.calculate_confidence(
            current_condition,
            vibration_rms_norm.abs(),
            temperature_norm.abs(),
        );

        let health_score = self.compute_health_score(
            vibration_rms_norm,
            temperature_norm,
            smoothed_rul,
            current_condition,
        );

        Ok(Some(RULPrediction {
            timestamp: chrono::Utc::now().timestamp(),
            machine_id,
            bearing_id: 1,
            rul_hours: smoothed_rul,
            rul_confidence,
            vibration_rms_trend,
            temperature_rate,
            skf_l10_life,
            lstm_prediction,
            health_score,
        }))
    }

    async fn smooth_rul(&self, machine_id: u16, raw_rul: f32) -> f32 {
        let mut states = self.machine_states.lock().await;
        let state = states.entry(machine_id).or_default();

        state.rul_smoothing_window.push(raw_rul);
        if state.rul_smoothing_window.len() > 10 {
            state.rul_smoothing_window.remove(0);
        }

        let smoothed = if state.rul_smoothing_window.len() >= 3 {
            let sorted: Vec<f32> = state.rul_smoothing_window.iter().copied().collect();
            let median = sorted[sorted.len() / 2];
            let last = state.last_rul;

            let max_change = last * 0.05;
            let delta = raw_rul - last;
            let clamped_delta = delta.clamp(-max_change, max_change);
            let filtered = last + clamped_delta;

            (median * 0.3 + filtered * 0.7).max(0.0)
        } else {
            raw_rul
        };

        state.last_rul = smoothed;
        smoothed
    }

    fn calculate_confidence(
        &self,
        condition: OperatingCondition,
        vib_abs: f32,
        temp_abs: f32,
    ) -> f32 {
        let condition_stability = match condition {
            OperatingCondition::Idle => 0.7,
            OperatingCondition::LowSpeedLowLoad => 0.85,
            OperatingCondition::MediumSpeedMediumLoad => 0.95,
            OperatingCondition::HighSpeedHighLoad => 0.9,
            OperatingCondition::Cutting => 0.8,
        };

        let feature_stability = 1.0 - (vib_abs + temp_abs) / 10.0;
        let confidence = (condition_stability * 0.6 + feature_stability * 0.4).clamp(0.6, 0.98);
        confidence
    }

    async fn calculate_vibration_trend(&self, machine_id: u16, normalized_rms: f32) -> f32 {
        let mut states = self.machine_states.lock().await;
        let state = states.entry(machine_id).or_default();

        state.feature_history.push(NormalizedFeatures {
            vibration_rms_norm: normalized_rms,
            temperature_norm: 0.0,
            speed_norm: 0.0,
            load_norm: 0.0,
            condition: OperatingCondition::MediumSpeedMediumLoad,
        });

        if state.feature_history.len() > 50 {
            state.feature_history.remove(0);
        }

        if state.feature_history.len() < 10 {
            return (normalized_rms * 20.0).max(0.0);
        }

        let recent: Vec<f32> = state.feature_history
            .iter()
            .rev()
            .take(10)
            .map(|f| f.vibration_rms_norm)
            .collect();

        let older: Vec<f32> = state.feature_history
            .iter()
            .take(state.feature_history.len().saturating_sub(10))
            .rev()
            .take(10)
            .map(|f| f.vibration_rms_norm)
            .collect();

        let recent_avg = recent.iter().sum::<f32>() / recent.len() as f32;
        let older_avg = if older.is_empty() { recent_avg } else { older.iter().sum::<f32>() / older.len() as f32 };

        let trend = (recent_avg - older_avg) * 50.0;
        trend.max(0.0)
    }

    async fn calculate_temperature_rate(&self, machine_id: u16, normalized_temp: f32) -> f32 {
        let mut states = self.machine_states.lock().await;
        let state = states.entry(machine_id).or_default();

        if let Some(last) = state.feature_history.last_mut() {
            last.temperature_norm = normalized_temp;
        }

        (normalized_temp * 15.0).max(0.0)
    }

    fn calculate_skf_l10(
        &self,
        vibration_norm: f32,
        temperature_norm: f32,
        load_norm: f32,
        condition: OperatingCondition,
    ) -> f32 {
        let condition_factor = match condition {
            OperatingCondition::Idle => 1.5,
            OperatingCondition::LowSpeedLowLoad => 1.2,
            OperatingCondition::MediumSpeedMediumLoad => 1.0,
            OperatingCondition::HighSpeedHighLoad => 0.8,
            OperatingCondition::Cutting => 0.6,
        };

        let load_factor = 1.0 + (vibration_norm * 0.15 + load_norm * 0.1).max(0.0);
        let temperature_factor = 1.0 + (temperature_norm * 0.08).max(0.0);

        let adjusted_dynamic_load = self.bearing_params.dynamic_load_rating
            / (load_factor * temperature_factor * condition_factor);

        let l10_life = (adjusted_dynamic_load / self.bearing_params.equivalent_load)
            .powf(self.bearing_params.life_exponent)
            * self.bearing_params.rated_life;

        l10_life.max(0.0)
    }

    async fn simulate_lstm_prediction(
        &self,
        machine_id: u16,
        vib_norm: f32,
        temp_norm: f32,
        speed_norm: f32,
        load_norm: f32,
        condition: OperatingCondition,
        skf_life: f32,
    ) -> f32 {
        let condition_weight = match condition {
            OperatingCondition::Idle => 0.3,
            OperatingCondition::LowSpeedLowLoad => 0.5,
            OperatingCondition::MediumSpeedMediumLoad => 1.0,
            OperatingCondition::HighSpeedHighLoad => 0.8,
            OperatingCondition::Cutting => 0.6,
        };

        let vib_impact = (vib_norm * 0.4).max(0.0);
        let temp_impact = (temp_norm * 0.25).max(0.0);
        let speed_impact = (speed_norm.abs() * 0.15);
        let load_impact = (load_norm * 0.2).max(0.0);

        let total_degradation = (vib_impact + temp_impact + speed_impact + load_impact) * condition_weight;
        let degradation_factor = 1.0 - total_degradation.min(0.9);

        let mut states = self.machine_states.lock().await;
        let state = states.entry(machine_id).or_default();
        state.last_condition = condition;
        state.condition_history.push(condition);
        if state.condition_history.len() > 100 {
            state.condition_history.remove(0);
        }

        let predicted_life = skf_life * degradation_factor.max(0.05);
        let noise = (rand::random::<f32>() - 0.5) * 100.0;

        (predicted_life + noise).max(0.0)
    }

    fn compute_health_score(
        &self,
        vibration_norm: f32,
        temperature_norm: f32,
        rul: f32,
        condition: OperatingCondition,
    ) -> u8 {
        let condition_baseline = match condition {
            OperatingCondition::Idle => 98,
            OperatingCondition::LowSpeedLowLoad => 95,
            OperatingCondition::MediumSpeedMediumLoad => 90,
            OperatingCondition::HighSpeedHighLoad => 85,
            OperatingCondition::Cutting => 80,
        };

        let vibration_penalty = (vibration_norm.max(0.0) * 8.0) as u8;
        let temperature_penalty = (temperature_norm.max(0.0) * 5.0) as u8;

        let rul_score = if rul > 5000.0 {
            95
        } else if rul > 2000.0 {
            85
        } else if rul > 500.0 {
            70
        } else if rul > 200.0 {
            50
        } else {
            30
        };

        let base_score = condition_baseline.saturating_sub(vibration_penalty).saturating_sub(temperature_penalty);
        let overall = (base_score as u16 * 7 + rul_score as u16 * 3) / 10;

        overall.clamp(0, 100) as u8
    }

    pub fn calculate_health_score(
        &self,
        machine_id: u16,
        sensor_data: &[&SensorData],
        rul: &RULPrediction,
    ) -> anyhow::Result<HealthScore> {
        let vibration_data: Vec<f32> = sensor_data
            .iter()
            .filter(|d| d.sensor_type == SensorType::Vibration)
            .map(|d| d.value.abs())
            .collect();

        let temperature_data: Vec<f32> = sensor_data
            .iter()
            .filter(|d| d.sensor_type == SensorType::Temperature)
            .map(|d| d.value)
            .collect();

        let displacement_data: Vec<f32> = sensor_data
            .iter()
            .filter(|d| d.sensor_type == SensorType::Displacement)
            .map(|d| d.value.abs())
            .collect();

        let spindle_speeds: Vec<f32> = sensor_data.iter().map(|d| d.spindle_speed).collect();
        let loads: Vec<f32> = sensor_data.iter().map(|d| d.load).collect();

        let avg_speed = if spindle_speeds.is_empty() { 0.0 } else { spindle_speeds.iter().sum::<f32>() / spindle_speeds.len() as f32 };
        let avg_load = if loads.is_empty() { 0.0 } else { loads.iter().sum::<f32>() / loads.len() as f32 };
        let condition = Self::classify_operating_condition(avg_speed, avg_load);
        let norm_params = self.get_normalization_params(condition);

        let vibration_rms = if vibration_data.is_empty() { 0.0 } else { calculate_rms(&vibration_data) };
        let avg_temp = if temperature_data.is_empty() { 45.0 } else { temperature_data.iter().sum::<f32>() / temperature_data.len() as f32 };
        let avg_disp = if displacement_data.is_empty() { 0.0 } else { displacement_data.iter().sum::<f32>() / displacement_data.len() as f32 };

        let vib_norm = Self::normalize_feature(vibration_rms, norm_params.vibration_rms_mean, norm_params.vibration_rms_std);
        let temp_norm = Self::normalize_feature(avg_temp, norm_params.temperature_mean, norm_params.temperature_std);

        let vibration_score = if vib_norm < 0.5 {
            95
        } else if vib_norm < 1.5 {
            80
        } else if vib_norm < 2.5 {
            60
        } else {
            40
        };

        let temperature_score = if temp_norm < 0.5 {
            95
        } else if temp_norm < 1.5 {
            80
        } else if temp_norm < 2.5 {
            60
        } else {
            40
        };

        let displacement_score = if avg_disp < 0.05 {
            95
        } else if avg_disp < 0.15 {
            85
        } else if avg_disp < 0.3 {
            70
        } else {
            50
        };

        let rul_score = if rul.rul_hours > 5000.0 {
            95
        } else if rul.rul_hours > 2000.0 {
            85
        } else if rul.rul_hours > 500.0 {
            70
        } else if rul.rul_hours > 200.0 {
            50
        } else {
            35
        };

        let overall_score = ((vibration_score as u16 * 4 + temperature_score as u16 * 3 + displacement_score as u16 * 1 + rul_score as u16 * 2) / 10) as u8;

        Ok(HealthScore {
            timestamp: chrono::Utc::now().timestamp(),
            machine_id,
            overall_score,
            vibration_score: vibration_score as u8,
            temperature_score: temperature_score as u8,
            displacement_score: displacement_score as u8,
            rul_score: rul_score as u8,
        })
    }
}

fn calculate_rms(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let sum_squares: f32 = data.iter().map(|x| x * x).sum();
    (sum_squares / data.len() as f32).sqrt()
}
