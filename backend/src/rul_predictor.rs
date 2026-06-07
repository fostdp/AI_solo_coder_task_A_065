use crate::config::Config;
use crate::models::*;
use chrono::{DateTime, Utc, Duration};
use log::{info, debug};
use std::collections::VecDeque;
use std::sync::Arc;
use dashmap::DashMap;
use parking_lot::Mutex;

#[derive(Clone)]
pub struct RULPredictor {
    config: Arc<Config>,
    machine_history: Arc<DashMap<u16, MachineHistory>>,
    bearing_params: BearingParams,
}

#[derive(Clone)]
struct MachineHistory {
    rms_values: VecDeque<(DateTime<Utc>, f64)>,
    temp_values: VecDeque<(DateTime<Utc>, f64)>,
    rul_estimates: VecDeque<(DateTime<Utc>, f64)>,
    base_life_hours: f64,
    operating_hours: f64,
}

struct BearingParams {
    rated_life: f64,
    dynamic_load_rating: f64,
    equivalent_load: f64,
    life_exponent: f64,
    base_rated_speed: f64,
    thermal_factor: f64,
}

impl Default for BearingParams {
    fn default() -> Self {
        Self {
            rated_life: 20000.0,
            dynamic_load_rating: 38.5,
            equivalent_load: 5.0,
            life_exponent: 3.0,
            base_rated_speed: 18000.0,
            thermal_factor: 0.02,
        }
    }
}

impl RULPredictor {
    pub fn new(config: Arc<Config>) -> Self {
        let machine_history = Arc::new(DashMap::new());
        
        for machine_id in 1..=config.machine_count as u16 {
            machine_history.insert(machine_id, MachineHistory {
                rms_values: VecDeque::with_capacity(1000),
                temp_values: VecDeque::with_capacity(1000),
                rul_estimates: VecDeque::with_capacity(100),
                base_life_hours: 20000.0 + (rand::random::<f64>() - 0.5) * 2000.0,
                operating_hours: (rand::random::<f64>() * 5000.0),
            });
        }

        Self {
            config,
            machine_history,
            bearing_params: BearingParams::default(),
        }
    }

    pub fn update(&self, data: &SensorData) -> RULPrediction {
        let mut history = self.machine_history.entry(data.machine_id)
            .or_insert_with(|| MachineHistory {
                rms_values: VecDeque::with_capacity(1000),
                temp_values: VecDeque::with_capacity(1000),
                rul_estimates: VecDeque::with_capacity(100),
                base_life_hours: 20000.0,
                operating_hours: 0.0,
            });

        let avg_rms = data.vibration.iter().map(|v| v.rms).sum::<f64>() / data.vibration.len() as f64;
        let avg_temp = data.temperature.iter().map(|t| t.value).sum::<f64>() / data.temperature.len() as f64;

        history.rms_values.push_back((data.timestamp, avg_rms));
        history.temp_values.push_back((data.timestamp, avg_temp));

        while history.rms_values.len() > 1000 {
            history.rms_values.pop_front();
        }
        while history.temp_values.len() > 1000 {
            history.temp_values.pop_front();
        }

        history.operating_hours += 0.1 / 3600.0;

        let rms_trend = self.calculate_rms_trend(&history.rms_values);
        let temp_rate = self.calculate_temp_rate(&history.temp_values);
        let skf_life = self.calculate_skf_life(avg_rms, avg_temp, data.spindle_speed);
        let lstm_adjustment = self.lstm_predict_adjustment(rms_trend, temp_rate, &history.rul_estimates);

        let adjusted_rul = (skf_life - history.operating_hours) * lstm_adjustment;
        let final_rul = adjusted_rul.max(0.0).min(history.base_life_hours);
        let confidence = self.calculate_confidence(&history.rms_values, &history.temp_values);

        let prediction = RULPrediction {
            machine_id: data.machine_id,
            timestamp: data.timestamp,
            rul_hours: final_rul,
            confidence,
            avg_rms,
            temp_rate,
            bearing_life_hours: skf_life,
        };

        history.rul_estimates.push_back((data.timestamp, final_rul));
        while history.rul_estimates.len() > 100 {
            history.rul_estimates.pop_front();
        }

        debug!("机床 {} RUL预测: {:.1}小时, 置信度: {:.2}%", data.machine_id, final_rul, confidence * 100.0);

        prediction
    }

    fn calculate_skf_life(&self, rms: f64, temp: f64, speed: f64) -> f64 {
        let load_factor = 1.0 + (rms / self.config.vibration_alarm_threshold).powf(2.0) * 0.5;
        let speed_factor = if speed > 0.0 {
            (speed / self.bearing_params.base_rated_speed).powf(0.7)
        } else {
            0.1
        };
        
        let temp_penalty = if temp > 60.0 {
            1.0 + self.bearing_params.thermal_factor * (temp - 60.0).powf(1.5)
        } else {
            1.0
        };

        let adjusted_equivalent_load = self.bearing_params.equivalent_load * load_factor;
        let basic_rating_life = (self.bearing_params.dynamic_load_rating / adjusted_equivalent_load)
            .powf(self.bearing_params.life_exponent) * 1e6;

        let life_hours = basic_rating_life / (60.0 * speed.max(100.0)) / temp_penalty / speed_factor;
        
        life_hours.min(self.bearing_params.rated_life * 1.5).max(100.0)
    }

    fn calculate_rms_trend(&self, rms_values: &VecDeque<(DateTime<Utc>, f64)>) -> f64 {
        if rms_values.len() < 10 {
            return 0.0;
        }

        let n = rms_values.len().min(100);
        let recent: Vec<_> = rms_values.iter().rev().take(n).collect();
        
        let sum_x: f64 = (0..n).sum::<usize>() as f64;
        let sum_y: f64 = recent.iter().map(|(_, y)| *y).sum();
        let sum_xy: f64 = recent.iter().enumerate().map(|(i, (_, y))| i as f64 * y).sum();
        let sum_x2: f64 = (0..n).map(|i| (i as f64).powi(2)).sum();

        let slope = (n as f64 * sum_xy - sum_x * sum_y) / (n as f64 * sum_x2 - sum_x.powi(2));
        
        slope / 0.001
    }

    fn calculate_temp_rate(&self, temp_values: &VecDeque<(DateTime<Utc>, f64)>) -> f64 {
        if temp_values.len() < 10 {
            return 0.0;
        }

        let n = temp_values.len().min(50);
        let recent: Vec<_> = temp_values.iter().rev().take(n).collect();
        
        if let (Some(first), Some(last)) = (recent.first(), recent.last()) {
            let dt = (last.0 - first.0).num_seconds().max(1) as f64 / 3600.0;
            (last.1 - first.1) / dt
        } else {
            0.0
        }
    }

    fn lstm_predict_adjustment(
        &self,
        rms_trend: f64,
        temp_rate: f64,
        history: &VecDeque<(DateTime<Utc>, f64)>
    ) -> f64 {
        let rms_factor = 1.0 - (rms_trend.max(-5.0).min(5.0) * 0.03);
        let temp_factor = 1.0 - (temp_rate.max(-10.0).min(10.0) * 0.01);
        
        let trend_factor = if history.len() >= 5 {
            let recent: Vec<_> = history.iter().rev().take(5).collect();
            if let (Some(first), Some(last)) = (recent.first(), recent.last()) {
                let ratio = last.1 / first.1.max(1.0);
                ratio.powf(0.1).max(0.5).min(1.5)
            } else {
                1.0
            }
        } else {
            1.0
        };

        let combined = rms_factor * temp_factor * trend_factor;
        combined.max(0.1).min(1.5)
    }

    fn calculate_confidence(
        &self,
        rms_values: &VecDeque<(DateTime<Utc>, f64)>,
        temp_values: &VecDeque<(DateTime<Utc>, f64)>
    ) -> f64 {
        let base_confidence = 0.7;
        
        let data_points = rms_values.len().min(temp_values.len());
        let data_factor = (data_points as f64 / 100.0).min(1.0) * 0.15;
        
        let rms_std = if rms_values.len() > 1 {
            let mean: f64 = rms_values.iter().map(|(_, v)| *v).sum::<f64>() / rms_values.len() as f64;
            let variance: f64 = rms_values.iter().map(|(_, v)| (v - mean).powi(2)).sum::<f64>() / rms_values.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };
        let stability_factor = (1.0 - (rms_std / 2.0).min(1.0)) * 0.15;

        (base_confidence + data_factor + stability_factor).max(0.3).min(0.98)
    }

    pub fn calculate_health_score(&self, machine_id: u16, max_rms: f64, max_temp: f64, rul: f64) -> f64 {
        let vibration_score = if max_rms < self.config.vibration_warning_threshold {
            100.0
        } else if max_rms < self.config.vibration_alarm_threshold {
            100.0 - (max_rms - self.config.vibration_warning_threshold) 
                / (self.config.vibration_alarm_threshold - self.config.vibration_warning_threshold) * 30.0
        } else {
            70.0 - (max_rms - self.config.vibration_alarm_threshold) * 5.0
        };

        let temp_score = if max_temp < 60.0 {
            100.0
        } else if max_temp < 80.0 {
            100.0 - (max_temp - 60.0) / 20.0 * 30.0
        } else {
            70.0 - (max_temp - 80.0) * 2.0
        };

        let rul_score = if rul > self.config.rul_warning_hours {
            100.0
        } else if rul > self.config.rul_alarm_hours {
            100.0 - (rul - self.config.rul_alarm_hours) 
                / (self.config.rul_warning_hours - self.config.rul_alarm_hours) * 30.0
        } else {
            70.0 - (self.config.rul_alarm_hours - rul) * 0.1
        };

        let score = vibration_score * 0.4 + temp_score * 0.3 + rul_score * 0.3;
        score.max(0.0).min(100.0)
    }

    pub fn should_generate_work_order(&self, rul: f64) -> bool {
        rul <= self.config.rul_warning_hours
    }
}
