use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tracing::{info, error, debug};
use chrono::Utc;
use ndarray::{Array1, Array2, Axis};
use rand::Rng;

use crate::config::Config;
use crate::models::{AppState, RULPrediction, MachineStatus};
use crate::clickhouse_client::ClickHouseClient;
use crate::alarm_manager::AlarmManager;

pub struct RULPredictor {
    config: Config,
}

impl RULPredictor {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn predict_rul(&self, vibration_trend: f64, temperature_trend: f64, 
                       runtime_hours: f64, avg_vibration: f64, avg_temp: f64) -> (f64, f64, String) {
        let skf_rul = self.calculate_skf_life(avg_vibration, avg_temp, runtime_hours);
        let lstm_rul = self.predict_lstm(vibration_trend, temperature_trend, avg_vibration, avg_temp);
        
        let combined_rul = 0.4 * skf_rul + 0.6 * lstm_rul;
        let health_score = self.calculate_health_score(combined_rul, avg_vibration);
        
        (combined_rul, health_score, "hybrid".to_string())
    }

    fn calculate_skf_life(&self, avg_vibration: f64, avg_temp: f64, runtime_hours: f64) -> f64 {
        let basic_rated_life = 20000.0;
        
        let vibration_factor = if avg_vibration < 2.8 {
            1.0
        } else if avg_vibration < 7.1 {
            1.0 - (avg_vibration - 2.8) / (7.1 - 2.8) * 0.3
        } else {
            0.7 - (avg_vibration - 7.1) / 10.0 * 0.5
        };

        let temp_factor = if avg_temp < 60.0 {
            1.0
        } else if avg_temp < 80.0 {
            1.0 - (avg_temp - 60.0) / 20.0 * 0.2
        } else {
            0.8 - (avg_temp - 80.0) / 40.0 * 0.4
        };

        let wear_factor = 1.0 - (runtime_hours / 50000.0).min(0.3);

        let adjusted_life = basic_rated_life * vibration_factor * temp_factor * wear_factor;
        let remaining = (adjusted_life - runtime_hours).max(10.0);

        remaining
    }

    fn predict_lstm(&self, vibration_trend: f64, temperature_trend: f64, 
                    avg_vibration: f64, avg_temp: f64) -> f64 {
        let mut rng = rand::thread_rng();
        let base_rul = 15000.0;

        let vibration_penalty = if vibration_trend > 0.0 {
            vibration_trend * 500.0
        } else {
            0.0
        };

        let temp_penalty = if temperature_trend > 0.0 {
            temperature_trend * 200.0
        } else {
            0.0
        };

        let severity_penalty = if avg_vibration > 7.1 {
            3000.0
        } else if avg_vibration > 2.8 {
            1000.0
        } else {
            0.0
        };

        let noise = rng.gen_range(-200.0..200.0);
        let predicted = base_rul - vibration_penalty - temp_penalty - severity_penalty + noise;

        predicted.max(50.0)
    }

    fn calculate_health_score(&self, rul_hours: f64, avg_vibration: f64) -> f64 {
        let rul_score = if rul_hours > 5000.0 {
            100.0
        } else if rul_hours > 1000.0 {
            60.0 + (rul_hours - 1000.0) / 4000.0 * 40.0
        } else if rul_hours > 200.0 {
            30.0 + (rul_hours - 200.0) / 800.0 * 30.0
        } else {
            rul_hours / 200.0 * 30.0
        };

        let vibration_score = if avg_vibration < 2.8 {
            100.0
        } else if avg_vibration < 7.1 {
            60.0 + (7.1 - avg_vibration) / (7.1 - 2.8) * 40.0
        } else {
            (15.0 - avg_vibration).max(0.0) / 15.0 * 60.0
        };

        0.6 * rul_score + 0.4 * vibration_score
    }

    pub fn calculate_trends(&self, metrics: &[crate::models::ProcessedMetrics]) -> (f64, f64, f64, f64) {
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

    info!("RUL prediction loop started, interval: {:?}", interval);

    loop {
        ticker.tick().await;
        
        debug!("Running RUL prediction for all machines");

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

            let (vib_trend, temp_trend, avg_vibration, avg_temp) = 
                rul_predictor.calculate_trends(&metrics);

            let runtime_hours = current_status.as_ref()
                .map(|s| s.total_runtime_hours)
                .unwrap_or(0.0);

            let (rul_hours, health_score, model_source) = 
                rul_predictor.predict_rul(vib_trend, temp_trend, runtime_hours, avg_vibration, avg_temp);

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
                    "主轴剩余寿命预测为{:.1}小时，低于预警阈值{}小时，建议安排轴承更换维护",
                    rul_hours, config.monitoring.rul_warning_threshold
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
