use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{self, Duration};
use tracing::{info, error, debug};
use chrono::Utc;
use rand::Rng;
use std::collections::HashMap;

use crate::config::Config;
use crate::models::{
    AppState, RULPredictionResult, MachineStatus, AnalyzedMetrics,
    OperatingCondition, NormalizationParams, ConditionFeatures,
};
use crate::clickhouse_client::ClickHouseClient;

const RPM_NORM_PARAMS: NormalizationParams = NormalizationParams {
    mean: 3000.0,
    std: 1500.0,
    min: 500.0,
    max: 8000.0,
};

struct ConditionNormParams {
    vibration_params: NormalizationParams,
    temp_params: NormalizationParams,
}

pub struct RULPredictor {
    config: Config,
    condition_models: HashMap<OperatingCondition, ConditionNormParams>,
    last_predictions: RwLock<HashMap<u16, f64>>,
    metrics_history: RwLock<HashMap<u16, Vec<AnalyzedMetrics>>>,
}

impl RULPredictor {
    pub fn new(config: &Config) -> (Arc<Self>, mpsc::Sender<AnalyzedMetrics>, mpsc::Receiver<RULPredictionResult>) {
        let (in_tx, in_rx) = mpsc::channel(1024);
        let (out_tx, out_rx) = mpsc::channel(512);

        let mut condition_models = HashMap::new();
        
        condition_models.insert(OperatingCondition::LowSpeed, ConditionNormParams {
            vibration_params: NormalizationParams::new(1.5, 0.5, 0.5, 5.0),
            temp_params: NormalizationParams::new(40.0, 8.0, 25.0, 70.0),
        });
        condition_models.insert(OperatingCondition::MediumSpeed, ConditionNormParams {
            vibration_params: NormalizationParams::new(2.5, 0.8, 1.0, 8.0),
            temp_params: NormalizationParams::new(50.0, 10.0, 30.0, 80.0),
        });
        condition_models.insert(OperatingCondition::HighSpeed, ConditionNormParams {
            vibration_params: NormalizationParams::new(3.5, 1.2, 1.5, 12.0),
            temp_params: NormalizationParams::new(60.0, 12.0, 35.0, 95.0),
        });
        condition_models.insert(OperatingCondition::Unknown, ConditionNormParams {
            vibration_params: NormalizationParams::new(2.5, 1.0, 1.0, 10.0),
            temp_params: NormalizationParams::new(50.0, 10.0, 30.0, 85.0),
        });

        let predictor = Arc::new(Self {
            config: config.clone(),
            condition_models,
            last_predictions: RwLock::new(HashMap::new()),
            metrics_history: RwLock::new(HashMap::new()),
        });

        let predictor_clone = predictor.clone();
        tokio::spawn(async move {
            if let Err(e) = predictor_clone.process_loop(in_rx, out_tx).await {
                error!("RULPredictor process loop error: {}", e);
            }
        });

        (predictor, in_tx, out_rx)
    }

    async fn process_loop(
        &self,
        mut in_rx: mpsc::Receiver<AnalyzedMetrics>,
        out_tx: mpsc::Sender<RULPredictionResult>,
    ) -> anyhow::Result<()> {
        let mut predict_interval = time::interval(
            Duration::from_secs(self.config.monitoring.rul_prediction_interval_sec)
        );
        let mut machine_ids: Vec<u16> = Vec::new();

        info!("RULPredictor: Processing loop started");

        loop {
            tokio::select! {
                Some(metrics) = in_rx.recv() => {
                    let mid = metrics.machine_id;
                    let mut history = self.metrics_history.write().await;
                    history.entry(mid).or_insert_with(Vec::new).push(metrics);
                    let history_vec = history.get_mut(&mid).unwrap();
                    if history_vec.len() > 500 {
                        history_vec.drain(0..history_vec.len() - 500);
                    }
                    if !machine_ids.contains(&mid) {
                        machine_ids.push(mid);
                    }
                }
                _ = predict_interval.tick() => {
                    debug!("RULPredictor: Running prediction batch for {} machines", machine_ids.len());
                    
                    for &machine_id in &machine_ids {
                        let history = self.metrics_history.read().await;
                        let metrics = history.get(&machine_id).cloned().unwrap_or_default();
                        drop(history);

                        if metrics.len() < 10 {
                            continue;
                        }

                        let runtime_hours = metrics.len() as f64 * 0.1 / 3600.0;
                        
                        match self.predict_single(machine_id, &metrics, runtime_hours).await {
                            Ok(result) => {
                                if let Err(e) = out_tx.send(result).await {
                                    warn!("RULPredictor: Failed to send prediction result: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("RULPredictor: Prediction failed for machine {}: {}", machine_id, e);
                            }
                        }
                    }
                }
            }
        }
    }

    async fn predict_single(
        &self,
        machine_id: u16,
        metrics: &[AnalyzedMetrics],
        runtime_hours: f64,
    ) -> anyhow::Result<RULPredictionResult> {
        let features = self.extract_condition_features(metrics);

        let model_params = self.condition_models.get(&features.condition)
            .unwrap_or_else(|| self.condition_models.get(&OperatingCondition::Unknown).unwrap());

        let vib_norm = model_params.vibration_params.normalize_z_score(features.vibration_mean);
        let temp_norm = model_params.temp_params.normalize_z_score(features.temp_mean);

        let skf_rul = self.calculate_skf_life_conditioned(
            features.vibration_mean, features.temp_mean,
            features.load_estimate, runtime_hours
        );
        
        let lstm_rul = self.predict_lstm_conditioned(
            vib_norm, temp_norm, features.temp_rate,
            features.load_estimate, features.rpm_normalized
        );

        let weights = match features.condition {
            OperatingCondition::LowSpeed => self.config.models.hybrid.weights_low_speed,
            OperatingCondition::MediumSpeed => self.config.models.hybrid.weights_medium_speed,
            OperatingCondition::HighSpeed => self.config.models.hybrid.weights_high_speed,
            _ => self.config.models.hybrid.weights_unknown,
        };

        let trend_factor = self.calculate_trend_factor(&features);
        
        let raw_prediction = weights[0] * skf_rul + weights[1] * lstm_rul + weights[2] * trend_factor;

        let smoothed = self.apply_smoothing(machine_id, raw_prediction).await;
        let clamped = smoothed.max(10.0).min(50000.0);
        
        let health_score = self.calculate_health_score(clamped, features.vibration_mean, features.condition);

        let (vib_trend, temp_trend, _, _) = self.calculate_trends(metrics);

        Ok(RULPredictionResult {
            timestamp: Utc::now(),
            machine_id,
            rul_hours: clamped,
            health_score,
            vibration_trend: vib_trend,
            temperature_trend: temp_trend,
            model_source: format!("hybrid_{}", features.condition.label()),
            condition: features.condition,
            skf_component: skf_rul,
            lstm_component: lstm_rul,
            trend_component: trend_factor,
        })
    }

    pub fn extract_condition_features(&self, metrics: &[AnalyzedMetrics]) -> ConditionFeatures {
        if metrics.is_empty() {
            return ConditionFeatures {
                condition: OperatingCondition::Unknown,
                rpm_normalized: 0.0,
                load_estimate: 0.5,
                vibration_mean: 0.0,
                vibration_std: 0.0,
                temp_mean: 0.0,
                temp_rate: 0.0,
            };
        }

        let rpms: Vec<f64> = metrics.iter().map(|m| m.rpm).collect();
        let avg_rpm = rpms.iter().sum::<f64>() / rpms.len() as f64;
        let condition = OperatingCondition::from_rpm(avg_rpm);

        let vib_values: Vec<f64> = metrics.iter()
            .flat_map(|m| &m.time_domain.rms)
            .copied()
            .collect();
        let vib_mean = vib_values.iter().sum::<f64>() / vib_values.len() as f64;
        let vib_std = self.compute_std(&vib_values, vib_mean);

        let temp_values: Vec<f64> = metrics.iter()
            .flat_map(|m| &m.temperature)
            .copied()
            .collect();
        let temp_mean = temp_values.iter().sum::<f64>() / temp_values.len() as f64;
        let temp_rate = self.compute_temperature_rate(metrics);

        let load_estimate = self.estimate_load(avg_rpm, vib_mean, temp_mean);
        let rpm_normalized = RPM_NORM_PARAMS.normalize_minmax(avg_rpm);

        ConditionFeatures {
            condition,
            rpm_normalized,
            load_estimate,
            vibration_mean: vib_mean,
            vibration_std: vib_std,
            temp_mean,
            temp_rate,
        }
    }

    fn estimate_load(&self, rpm: f64, vib_mean: f64, temp_mean: f64) -> f64 {
        let rpm_factor = RPM_NORM_PARAMS.normalize_minmax(rpm);
        let vib_factor = (vib_mean / 5.0).min(1.0);
        let temp_factor = ((temp_mean - 30.0) / 50.0).min(1.0).max(0.0);
        
        (0.4 * rpm_factor + 0.35 * vib_factor + 0.25 * temp_factor).min(1.0).max(0.0)
    }

    fn compute_std(&self, values: &[f64], mean: f64) -> f64 {
        if values.len() < 2 { return 0.0; }
        let variance: f64 = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        variance.sqrt()
    }

    fn compute_temperature_rate(&self, metrics: &[AnalyzedMetrics]) -> f64 {
        if metrics.len() < 10 { return 0.0; }
        let window = metrics.len().min(50);
        let first_half: Vec<f64> = metrics.iter().take(window / 2).flat_map(|m| &m.temperature).copied().collect();
        let second_half: Vec<f64> = metrics.iter().skip(window / 2).take(window / 2).flat_map(|m| &m.temperature).copied().collect();
        let first_mean = first_half.iter().sum::<f64>() / first_half.len() as f64;
        let second_mean = second_half.iter().sum::<f64>() / second_half.len() as f64;
        second_mean - first_mean
    }

    async fn apply_smoothing(&self, machine_id: u16, new_value: f64) -> f64 {
        let alpha = self.config.models.lstm.smoothing_factor;
        let mut last_preds = self.last_predictions.write().await;
        
        if let Some(&last) = last_preds.get(&machine_id) {
            let smoothed = alpha * new_value + (1.0 - alpha) * last;
            last_preds.insert(machine_id, smoothed);
            smoothed
        } else {
            last_preds.insert(machine_id, new_value);
            new_value
        }
    }

    fn calculate_skf_life_conditioned(&self, avg_vibration: f64, avg_temp: f64, load: f64, runtime_hours: f64) -> f64 {
        let skf = &self.config.models.skf;
        let basic = skf.basic_rated_life_hours;

        let vib_factor = if avg_vibration < skf.vibration_factor_low_vib_low {
            1.0
        } else if avg_vibration < skf.vib_factor_medium_low {
            1.0 - (avg_vibration - skf.vibration_factor_low_vib_low) / (skf.vib_factor_medium_low - skf.vibration_factor_low_vib_low) * 0.25
        } else if avg_vibration < skf.vib_factor_high {
            0.75 - (avg_vibration - skf.vib_factor_medium_low) / (skf.vib_factor_high - skf.vib_factor_medium_low) * 0.3
        } else {
            0.45 - (avg_vibration - skf.vib_factor_high) / 7.0 * 0.4
        };

        let temp_factor = if avg_temp < skf.temp_factor_low {
            1.0
        } else if avg_temp < skf.temp_factor_medium {
            1.0 - (avg_temp - skf.temp_factor_low) / (skf.temp_factor_medium - skf.temp_factor_low) * 0.2
        } else if avg_temp < skf.temp_factor_high {
            0.8 - (avg_temp - skf.temp_factor_medium) / (skf.temp_factor_high - skf.temp_factor_medium) * 0.35
        } else {
            0.45
        };

        let load_factor = 1.0 - load * skf.load_factor_coefficient;
        let wear_factor = 1.0 - (runtime_hours / skf.wear_rate_per_50k_hours).min(skf.max_wear_factor);

        let adjusted = basic * vib_factor * temp_factor * load_factor * wear_factor;
        (adjusted - runtime_hours).max(10.0)
    }

    fn predict_lstm_conditioned(&self, vib_norm: f64, temp_norm: f64, temp_rate: f64, load: f64, rpm_norm: f64) -> f64 {
        let lstm = &self.config.models.lstm;
        let mut rng = rand::thread_rng();

        let vib_penalty = if vib_norm > 0.0 {
            vib_norm * lstm.vib_high_penalty_per_std
        } else {
            vib_norm * lstm.vib_low_penalty_per_std
        };

        let temp_penalty = if temp_norm > 0.0 { temp_norm * lstm.temp_penalty_per_std } else { 0.0 };
        let rate_penalty = if temp_rate > 0.1 { temp_rate * lstm.temp_rate_penalty_per_deg_s } else { 0.0 };
        let load_penalty = load * lstm.load_penalty_coefficient;
        let rpm_penalty = if rpm_norm > lstm.high_rpm_threshold { (rpm_norm - lstm.high_rpm_threshold) * lstm.high_rpm_penalty } else { 0.0 };

        let noise: f64 = rng.gen_range(-lstm.noise_std_dev..lstm.noise_std_dev);
        
        let predicted = lstm.base_rul_hours - vib_penalty - temp_penalty - rate_penalty - load_penalty - rpm_penalty + noise;
        predicted.max(50.0)
    }

    fn calculate_trend_factor(&self, features: &ConditionFeatures) -> f64 {
        let vib_increasing = features.vibration_std > 0.8;
        let temp_increasing = features.temp_rate > 0.05;
        
        match (vib_increasing, temp_increasing) {
            (true, true) => -2000.0,
            (true, false) => -800.0,
            (false, true) => -500.0,
            (false, false) => 500.0,
        }
    }

    fn calculate_health_score(&self, rul_hours: f64, avg_vibration: f64, condition: OperatingCondition) -> f64 {
        let vib_threshold = match condition {
            OperatingCondition::LowSpeed => 5.0,
            OperatingCondition::MediumSpeed => 7.1,
            OperatingCondition::HighSpeed => 9.0,
            _ => 7.1,
        };

        let rul_score = if rul_hours > 8000.0 { 100.0 }
            else if rul_hours > 2000.0 { 60.0 + (rul_hours - 2000.0) / 6000.0 * 40.0 }
            else if rul_hours > 500.0 { 30.0 + (rul_hours - 500.0) / 1500.0 * 30.0 }
            else { rul_hours / 500.0 * 30.0 };

        let vib_norm = (avg_vibration / vib_threshold).min(1.5);
        let vibration_score = if vib_norm < 0.4 { 100.0 }
            else if vib_norm < 1.0 { 60.0 + (1.0 - vib_norm) / 0.6 * 40.0 }
            else { ((1.5 - vib_norm) / 0.5 * 60.0).max(0.0) };

        0.6 * rul_score + 0.4 * vibration_score
    }

    fn calculate_trends(&self, metrics: &[AnalyzedMetrics]) -> (f64, f64, f64, f64) {
        if metrics.len() < 2 { return (0.0, 0.0, 1.0, 30.0); }

        let rms_values: Vec<f64> = metrics.iter()
            .map(|m| m.time_domain.rms.iter().sum::<f64>() / m.time_domain.rms.len() as f64)
            .collect();
        
        let temp_values: Vec<f64> = metrics.iter()
            .map(|m| m.temperature.iter().sum::<f64>() / m.temperature.len() as f64)
            .collect();

        let n = rms_values.len() as f64;
        let sum_x: f64 = (0..rms_values.len()).map(|i| i as f64).sum();
        let sum_y_rms: f64 = rms_values.iter().sum();
        let sum_xy_rms: f64 = rms_values.iter().enumerate().map(|(i, &v)| i as f64 * v).sum();
        let sum_x2: f64 = (0..rms_values.len()).map(|i| (i as f64).powi(2)).sum();

        let vib_slope = (n * sum_xy_rms - sum_x * sum_y_rms) / (n * sum_x2 - sum_x.powi(2));

        let sum_y_temp: f64 = temp_values.iter().sum();
        let sum_xy_temp: f64 = temp_values.iter().enumerate().map(|(i, &v)| i as f64 * v).sum();
        let temp_slope = (n * sum_xy_temp - sum_x * sum_y_temp) / (n * sum_x2 - sum_x.powi(2));

        let avg_vibration = sum_y_rms / n;
        let avg_temp = sum_y_temp / n;

        (vib_slope, temp_slope, avg_vibration, avg_temp)
    }
}

pub async fn start_rul_prediction_loop(
    config: Config,
    app_state: Arc<RwLock<AppState>>,
    clickhouse: Arc<ClickHouseClient>,
    mut prediction_rx: mpsc::Receiver<RULPredictionResult>,
) -> anyhow::Result<()> {
    info!("RUL prediction result handler loop started");

    loop {
        if let Some(result) = prediction_rx.recv().await {
            if let Err(e) = clickhouse.insert_rul_prediction(&result.clone().into()).await {
                error!("Failed to insert RUL prediction for machine {}: {}", result.machine_id, e);
            }

            let mut state = app_state.write().await;
            if let Some(status) = state.machine_statuses.get_mut(&result.machine_id) {
                status.rul_hours = result.rul_hours;
                status.health_score = result.health_score;
                
                if result.rul_hours < config.monitoring.rul_alarm_threshold {
                    status.alarm_level = 2;
                } else if result.rul_hours < config.monitoring.rul_warning_threshold {
                    status.alarm_level = 1;
                } else {
                    status.alarm_level = 0;
                }

                if let Err(e) = clickhouse.update_machine_status(status).await {
                    error!("Failed to update machine status for {}: {}", result.machine_id, e);
                }
            }
        }
    }
}

impl From<RULPredictionResult> for crate::models::RULPrediction {
    fn from(r: RULPredictionResult) -> Self {
        crate::models::RULPrediction {
            timestamp: r.timestamp,
            machine_id: r.machine_id,
            rul_hours: r.rul_hours,
            health_score: r.health_score,
            vibration_trend: r.vibration_trend,
            temperature_trend: r.temperature_trend,
            model_source: r.model_source,
        }
    }
}
