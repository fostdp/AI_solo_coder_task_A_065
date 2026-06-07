use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tracing::{info, error, debug};
use chrono::Utc;
use rand::Rng;
use std::collections::HashMap;

use crate::config::Config;
use crate::models::{
    AppState, RULPrediction, MachineStatus, ProcessedMetrics,
    OperatingCondition, NormalizationParams, ConditionFeatures,
};
use crate::clickhouse_client::ClickHouseClient;
use crate::alarm_manager::AlarmManager;

const RPM_NORM_PARAMS: NormalizationParams = NormalizationParams {
    mean: 3000.0,
    std: 1500.0,
    min: 500.0,
    max: 8000.0,
};

struct ConditionNormParams {
    vibration_params: NormalizationParams,
    temp_params: NormalizationParams,
    rul_model_weights: (f64, f64, f64),
}

pub struct RULPredictor {
    config: Config,
    condition_models: HashMap<OperatingCondition, ConditionNormParams>,
    smoothing_factor: f64,
    last_predictions: RwLock<HashMap<u16, f64>>,
}

impl RULPredictor {
    pub fn new(config: &Config) -> Self {
        let mut condition_models = HashMap::new();
        
        condition_models.insert(OperatingCondition::LowSpeed, ConditionNormParams {
            vibration_params: NormalizationParams::new(1.5, 0.5, 0.5, 5.0),
            temp_params: NormalizationParams::new(40.0, 8.0, 25.0, 70.0),
            rul_model_weights: (0.35, 0.35, 0.30),
        });

        condition_models.insert(OperatingCondition::MediumSpeed, ConditionNormParams {
            vibration_params: NormalizationParams::new(2.5, 0.8, 1.0, 8.0),
            temp_params: NormalizationParams::new(50.0, 10.0, 30.0, 80.0),
            rul_model_weights: (0.40, 0.35, 0.25),
        });

        condition_models.insert(OperatingCondition::HighSpeed, ConditionNormParams {
            vibration_params: NormalizationParams::new(3.5, 1.2, 1.5, 12.0),
            temp_params: NormalizationParams::new(60.0, 12.0, 35.0, 95.0),
            rul_model_weights: (0.45, 0.35, 0.20),
        });

        condition_models.insert(OperatingCondition::Unknown, ConditionNormParams {
            vibration_params: NormalizationParams::new(2.5, 1.0, 1.0, 10.0),
            temp_params: NormalizationParams::new(50.0, 10.0, 30.0, 85.0),
            rul_model_weights: (0.40, 0.35, 0.25),
        });

        Self {
            config: config.clone(),
            condition_models,
            smoothing_factor: 0.3,
            last_predictions: RwLock::new(HashMap::new()),
        }
    }

    pub fn extract_condition_features(&self, metrics: &[ProcessedMetrics]) -> ConditionFeatures {
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
            .flat_map(|m| &m.vibration_rms)
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
        if values.len() < 2 {
            return 0.0;
        }
        let variance: f64 = values.iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        variance.sqrt()
    }

    fn compute_temperature_rate(&self, metrics: &[ProcessedMetrics]) -> f64 {
        if metrics.len() < 10 {
            return 0.0;
        }

        let window = metrics.len().min(50);
        let first_half: Vec<f64> = metrics.iter()
            .take(window / 2)
            .flat_map(|m| &m.temperature)
            .copied()
            .collect();
        let second_half: Vec<f64> = metrics.iter()
            .skip(window / 2)
            .take(window / 2)
            .flat_map(|m| &m.temperature)
            .copied()
            .collect();

        let first_mean = first_half.iter().sum::<f64>() / first_half.len() as f64;
        let second_mean = second_half.iter().sum::<f64>() / second_half.len() as f64;
        
        second_mean - first_mean
    }

    pub async fn predict_rul(&self, machine_id: u16, features: &ConditionFeatures, 
                            runtime_hours: f64) -> (f64, f64, String) {
        let model = self.condition_models.get(&features.condition)
            .unwrap_or_else(|| self.condition_models.get(&OperatingCondition::Unknown).unwrap());

        let vib_norm = model.vibration_params.normalize_z_score(features.vibration_mean);
        let temp_norm = model.temp_params.normalize_z_score(features.temp_mean);

        let skf_rul = self.calculate_skf_life_conditioned(
            features.vibration_mean, features.temp_mean, 
            features.load_estimate, runtime_hours
        );
        
        let lstm_rul = self.predict_lstm_conditioned(
            vib_norm, temp_norm, features.temp_rate,
            features.load_estimate, features.rpm_normalized
        );

        let (w_skf, w_lstm, w_trend) = model.rul_model_weights;
        let trend_factor = self.calculate_trend_factor(features);
        
        let raw_prediction = w_skf * skf_rul + w_lstm * lstm_rul + w_trend * trend_factor;

        let smoothed = self.apply_smoothing(machine_id, raw_prediction).await;
        let clamped = smoothed.max(10.0).min(50000.0);
        
        let health_score = self.calculate_health_score(clamped, features.vibration_mean, features.condition);

        (clamped, health_score, format!("hybrid_{}", features.condition.label()))
    }

    async fn apply_smoothing(&self, machine_id: u16, new_value: f64) -> f64 {
        let mut last_preds = self.last_predictions.write().await;
        
        if let Some(&last) = last_preds.get(&machine_id) {
            let smoothed = self.smoothing_factor * new_value + (1.0 - self.smoothing_factor) * last;
            last_preds.insert(machine_id, smoothed);
            smoothed
        } else {
            last_preds.insert(machine_id, new_value);
            new_value
        }
    }

    fn calculate_skf_life_conditioned(&self, avg_vibration: f64, avg_temp: f64, 
                                     load: f64, runtime_hours: f64) -> f64 {
        let basic_rated_life = 25000.0;
        
        let vib_factor = if avg_vibration < 2.0 {
            1.0
        } else if avg_vibration < 5.0 {
            1.0 - (avg_vibration - 2.0) / 3.0 * 0.25
        } else if avg_vibration < 8.0 {
            0.75 - (avg_vibration - 5.0) / 3.0 * 0.3
        } else {
            0.45 - (avg_vibration - 8.0) / 7.0 * 0.4
        };

        let temp_factor = if avg_temp < 50.0 {
            1.0
        } else if avg_temp < 70.0 {
            1.0 - (avg_temp - 50.0) / 20.0 * 0.2
        } else if avg_temp < 90.0 {
            0.8 - (avg_temp - 70.0) / 20.0 * 0.35
        } else {
            0.45
        };

        let load_factor = 1.0 - load * 0.15;
        let wear_factor = 1.0 - (runtime_hours / 50000.0).min(0.35);

        let adjusted_life = basic_rated_life * vib_factor * temp_factor * load_factor * wear_factor;
        (adjusted_life - runtime_hours).max(10.0)
    }

    fn predict_lstm_conditioned(&self, vib_norm: f64, temp_norm: f64, 
                                temp_rate: f64, load: f64, rpm_norm: f64) -> f64 {
        let mut rng = rand::thread_rng();
        let base_rul = 18000.0;

        let vib_penalty = if vib_norm > 0.0 {
            vib_norm * 800.0
        } else {
            vib_norm * 200.0
        };

        let temp_penalty = if temp_norm > 0.0 {
            temp_norm * 500.0
        } else {
            0.0
        };

        let rate_penalty = if temp_rate > 0.1 {
            temp_rate * 2000.0
        } else {
            0.0
        };

        let load_penalty = load * 1500.0;
        let rpm_penalty = if rpm_norm > 0.7 {
            (rpm_norm - 0.7) * 1000.0
        } else {
            0.0
        };

        let noise = rng.gen_range(-100.0..100.0);
        
        let predicted = base_rul - vib_penalty - temp_penalty 
            - rate_penalty - load_penalty - rpm_penalty + noise;

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

    fn calculate_health_score(&self, rul_hours: f64, avg_vibration: f64, 
                             condition: OperatingCondition) -> f64 {
        let vib_threshold = match condition {
            OperatingCondition::LowSpeed => 5.0,
            OperatingCondition::MediumSpeed => 7.1,
            OperatingCondition::HighSpeed => 9.0,
            _ => 7.1,
        };

        let rul_score = if rul_hours > 8000.0 {
            100.0
        } else if rul_hours > 2000.0 {
            60.0 + (rul_hours - 2000.0) / 6000.0 * 40.0
        } else if rul_hours > 500.0 {
            30.0 + (rul_hours - 500.0) / 1500.0 * 30.0
        } else {
            rul_hours / 500.0 * 30.0
        };

        let vib_normalized = (avg_vibration / vib_threshold).min(1.5);
        let vibration_score = if vib_normalized < 0.4 {
            100.0
        } else if vib_normalized < 1.0 {
            60.0 + (1.0 - vib_normalized) / 0.6 * 40.0
        } else {
            ((1.5 - vib_normalized) / 0.5 * 60.0).max(0.0)
        };

        0.6 * rul_score + 0.4 * vibration_score
    }

    pub fn calculate_trends(&self, metrics: &[ProcessedMetrics]) -> (f64, f64, f64, f64) {
        if metrics.len() < 2 {
            return (0.0, 0.0, 1.0, 30.0);
        }

        let rms_values: Vec<f64> = metrics.iter()
            .map(|m| m.vibration_rms.iter().sum::<f64>() / m.vibration_rms.len() as f64)
            .collect();
        
        let temp_values: Vec<f64> = metrics.iter()
            .map(|m| m.temperature.iter().sum::<f64>() / m.temperature.len() as f64)
            .collect();

        let n = rms_values.len() as f64;
        let sum_x: f64 = (0..rms_values.len()).map(|i| i as f64).sum();
        let sum_y_rms: f64 = rms_values.iter().sum();
        let sum_xy_rms: f64 = rms_values.iter().enumerate()
            .map(|(i, &v)| i as f64 * v).sum();
        let sum_x2: f64 = (0..rms_values.len()).map(|i| (i as f64).powi(2)).sum();

        let vib_slope = (n * sum_xy_rms - sum_x * sum_y_rms) / (n * sum_x2 - sum_x.powi(2));

        let sum_y_temp: f64 = temp_values.iter().sum();
        let sum_xy_temp: f64 = temp_values.iter().enumerate()
            .map(|(i, &v)| i as f64 * v).sum();

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
    rul_predictor: Arc<RULPredictor>,
    alarm_manager: Arc<AlarmManager>,
) -> anyhow::Result<()> {
    let interval = Duration::from_secs(config.monitoring.rul_prediction_interval_sec);
    let mut ticker = time::interval(interval);

    info!("RUL prediction loop started, interval: {:?}, condition-aware model enabled", interval);

    loop {
        ticker.tick().await;
        
        debug!("Running RUL prediction for all machines with condition normalization");

        let state = app_state.read().await;
        let machine_ids: Vec<u16> = state.machine_statuses.keys().cloned().collect();
        drop(state);

        for machine_id in machine_ids {
            let state = app_state.read().await;
            let metrics = state.recent_metrics.get(&machine_id)
                .cloned()
                .unwrap_or_default();
            let current_status = state.machine_statuses.get(&machine_id).cloned();
            drop(state);

            if metrics.len() < 10 {
                continue;
            }

            let features = rul_predictor.extract_condition_features(&metrics);
            
            let runtime_hours = current_status.as_ref()
                .map(|s| s.total_runtime_hours)
                .unwrap_or(0.0);

            let (rul_hours, health_score, model_source) = 
                rul_predictor.predict_rul(machine_id, &features, runtime_hours).await;

            let (vib_trend, temp_trend, _, _) = rul_predictor.calculate_trends(&metrics);

            let prediction = RULPrediction {
                timestamp: Utc::now(),
                machine_id,
                rul_hours,
                health_score,
                vibration_trend: vib_trend,
                temperature_trend: temp_trend,
                model_source,
            };

            if let Err(e) = clickhouse.insert_rul_prediction(&prediction).await {
                error!("Failed to insert RUL prediction for machine {}: {}", machine_id, e);
            }

            let mut state = app_state.write().await;
            if let Some(status) = state.machine_statuses.get_mut(&machine_id) {
                status.rul_hours = rul_hours;
                status.health_score = health_score;
                
                if rul_hours < config.monitoring.rul_alarm_threshold {
                    status.alarm_level = 2;
                } else if rul_hours < config.monitoring.rul_warning_threshold 
                        || status.vibration_severity.iter().any(|&v| v > config.monitoring.vibration_alarm) {
                    status.alarm_level = 1;
                } else {
                    status.alarm_level = 0;
                }

                if let Err(e) = clickhouse.update_machine_status(status).await {
                    error!("Failed to update machine status for {}: {}", machine_id, e);
                }
            }
            drop(state);

            alarm_manager.check_rul_alarm(machine_id, rul_hours).await;

            if rul_hours < config.monitoring.rul_warning_threshold {
                let description = format!(
                    "[{}工况] 主轴剩余寿命预测为{:.1}小时，低于预警阈值{}小时，建议安排轴承更换维护",
                    features.condition.label(), rul_hours, config.monitoring.rul_warning_threshold
                );
                
                if let Err(e) = clickhouse.create_maintenance_order(
                    machine_id, &description, rul_hours
                ).await {
                    error!("Failed to create maintenance order for machine {}: {}", machine_id, e);
                }
            }
        }
    }
}
