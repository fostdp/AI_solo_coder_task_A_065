use std::sync::Arc;
use crate::models::*;
use crate::clickhouse_client::ClickHouseClient;
use log::info;

pub struct PredictionEngine {
    clickhouse: ClickHouseClient,
    bearing_params: BearingParams,
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

impl PredictionEngine {
    pub fn new(clickhouse: ClickHouseClient) -> Self {
        PredictionEngine {
            clickhouse,
            bearing_params: BearingParams::default(),
        }
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

        if vibration_data.is_empty() || temperature_data.is_empty() {
            return Ok(None);
        }

        let vibration_rms = calculate_rms(&vibration_data);
        let vibration_rms_trend = self.calculate_vibration_trend(machine_id, vibration_rms).await;
        let temperature_rate = self.calculate_temperature_rate(machine_id, &temperature_data).await;

        let skf_l10_life = self.calculate_skf_l10(vibration_rms, temperature_rate);
        let lstm_prediction = self.simulate_lstm_prediction(vibration_rms_trend, temperature_rate, skf_l10_life);

        let health_score = self.compute_health_score(vibration_rms, temperature_rate, skf_l10_life);

        let rul_hours = (skf_l10_life * 0.4 + lstm_prediction * 0.6).max(0.0);
        let rul_confidence = if rul_hours > 2000.0 { 0.95 } else { 0.85 };

        Ok(Some(RULPrediction {
            timestamp: chrono::Utc::now().timestamp(),
            machine_id,
            bearing_id: 1,
            rul_hours,
            rul_confidence,
            vibration_rms_trend,
            temperature_rate,
            skf_l10_life,
            lstm_prediction,
            health_score,
        }))
    }

    async fn calculate_vibration_trend(&self, machine_id: u16, current_rms: f32) -> f32 {
        let baseline = 1.5;
        let trend = (current_rms - baseline) / baseline * 100.0;
        trend.max(0.0)
    }

    async fn calculate_temperature_rate(&self, machine_id: u16, current_temps: &[f32]) -> f32 {
        if current_temps.len() < 2 {
            return 0.0;
        }
        let avg_temp: f32 = current_temps.iter().sum::<f32>() / current_temps.len() as f32;
        let baseline_temp = 45.0;
        (avg_temp - baseline_temp) / baseline_temp * 100.0
    }

    fn calculate_skf_l10(&self, vibration_rms: f32, temperature_rate: f32) -> f32 {
        let load_factor = 1.0 + vibration_rms / 10.0;
        let temperature_factor = 1.0 + temperature_rate / 50.0;
        
        let adjusted_dynamic_load = self.bearing_params.dynamic_load_rating / (load_factor * temperature_factor);
        
        let l10_life = (adjusted_dynamic_load / self.bearing_params.equivalent_load)
            .powf(self.bearing_params.life_exponent)
            * self.bearing_params.rated_life;
        
        l10_life.max(0.0)
    }

    fn simulate_lstm_prediction(&self, vibration_trend: f32, temperature_rate: f32, skf_life: f32) -> f32 {
        let degradation_factor = 1.0 - (vibration_trend / 100.0 * 0.5 + temperature_rate / 100.0 * 0.3);
        let predicted_life = skf_life * degradation_factor.max(0.1);
        
        let noise = (rand::random::<f32>() - 0.5) * 200.0;
        (predicted_life + noise).max(0.0)
    }

    fn compute_health_score(&self, vibration_rms: f32, temperature_rate: f32, skf_life: f32) -> u8 {
        let vibration_score = if vibration_rms < 2.8 {
            100.0
        } else if vibration_rms < 7.1 {
            100.0 - (vibration_rms - 2.8) / (7.1 - 2.8) * 30.0
        } else {
            70.0 - (vibration_rms - 7.1) / 10.0 * 70.0
        };

        let temperature_score = if temperature_rate < 10.0 {
            100.0
        } else if temperature_rate < 30.0 {
            100.0 - (temperature_rate - 10.0) / 20.0 * 30.0
        } else {
            70.0 - (temperature_rate - 30.0) / 50.0 * 70.0
        };

        let life_score = if skf_life > 5000.0 {
            100.0
        } else if skf_life > 1000.0 {
            100.0 - (5000.0 - skf_life) / 4000.0 * 30.0
        } else {
            70.0 - (1000.0 - skf_life) / 1000.0 * 70.0
        };

        let overall = vibration_score * 0.4 + temperature_score * 0.3 + life_score * 0.3;
        overall.clamp(0.0, 100.0) as u8
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

        let vibration_rms = if vibration_data.is_empty() { 0.0 } else { calculate_rms(&vibration_data) };
        let avg_temp = if temperature_data.is_empty() { 45.0 } else { temperature_data.iter().sum::<f32>() / temperature_data.len() as f32 };
        let avg_disp = if displacement_data.is_empty() { 0.0 } else { displacement_data.iter().sum::<f32>() / displacement_data.len() as f32 };

        let vibration_score = if vibration_rms < 2.8 {
            95
        } else if vibration_rms < 7.1 {
            80
        } else {
            60
        };

        let temperature_score = if avg_temp < 55.0 {
            95
        } else if avg_temp < 75.0 {
            80
        } else {
            60
        };

        let displacement_score = if avg_disp < 0.1 {
            95
        } else if avg_disp < 0.3 {
            80
        } else {
            60
        };

        let rul_score = if rul.rul_hours > 2000.0 {
            95
        } else if rul.rul_hours > 500.0 {
            80
        } else if rul.rul_hours > 200.0 {
            60
        } else {
            40
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
