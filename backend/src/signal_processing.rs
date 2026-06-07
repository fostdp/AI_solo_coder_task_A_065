use rustfft::{FftPlanner, num_complex::Complex};
use chrono::{Utc, DateTime};
use rand::Rng;

use crate::models::{SensorData, ProcessedMetrics};

pub struct SignalProcessor {
    fft_planner: std::sync::Mutex<FftPlanner<f64>>,
}

impl SignalProcessor {
    pub fn new() -> Self {
        Self {
            fft_planner: std::sync::Mutex::new(FftPlanner::new()),
        }
    }

    pub fn process_metrics(&self, data: &SensorData) -> ProcessedMetrics {
        let timestamp: DateTime<Utc> = Utc::now();

        let vibration_rms: Vec<f64> = data.vibration.iter()
            .map(|&v| (v * v).sqrt())
            .collect();

        let vibration_peak: Vec<f64> = data.vibration.iter()
            .map(|&v| v.abs())
            .collect();

        let vibration_freq: Vec<Vec<f64>> = data.vibration.chunks(128)
            .map(|chunk| self.compute_fft(chunk))
            .collect();

        ProcessedMetrics {
            timestamp,
            machine_id: data.machine_id,
            spindle_id: data.spindle_id,
            vibration: data.vibration.clone(),
            temperature: data.temperature.clone(),
            displacement: data.displacement.clone(),
            rpm: data.rpm,
            vibration_rms,
            vibration_peak,
            vibration_freq,
        }
    }

    fn compute_fft(&self, signal: &[f64]) -> Vec<f64> {
        let n = signal.len().max(64).next_power_of_two();
        let mut buffer: Vec<Complex<f64>> = signal.iter()
            .take(n)
            .map(|&v| Complex::new(v, 0.0))
            .chain(std::iter::repeat(Complex::new(0.0, 0.0)))
            .take(n)
            .collect();

        let mut planner = self.fft_planner.lock().unwrap();
        let fft = planner.plan_fft_forward(n);
        fft.process(&mut buffer);

        buffer.iter()
            .take(n / 2)
            .map(|c| c.norm() / n as f64)
            .collect()
    }

    pub fn compute_rms(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = values.iter().map(|v| v * v).sum();
        (sum_sq / values.len() as f64).sqrt()
    }

    pub fn compute_peak_to_peak(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        max - min
    }

    pub fn compute_crest_factor(&self, values: &[f64]) -> f64 {
        let rms = self.compute_rms(values);
        if rms == 0.0 {
            return 0.0;
        }
        let peak = values.iter().map(|&v| v.abs()).fold(0.0, f64::max);
        peak / rms
    }

    pub fn compute_skewness(&self, values: &[f64]) -> f64 {
        if values.len() < 3 {
            return 0.0;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let n = values.len() as f64;
        
        let m3: f64 = values.iter().map(|&v| (v - mean).powi(3)).sum::<f64>() / n;
        let m2: f64 = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / n;
        
        if m2 == 0.0 {
            return 0.0;
        }
        m3 / m2.powf(1.5)
    }

    pub fn compute_kurtosis(&self, values: &[f64]) -> f64 {
        if values.len() < 4 {
            return 0.0;
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let n = values.len() as f64;
        
        let m4: f64 = values.iter().map(|&v| (v - mean).powi(4)).sum::<f64>() / n;
        let m2: f64 = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / n;
        
        if m2 == 0.0 {
            return 0.0;
        }
        m4 / m2.powi(2) - 3.0
    }
}

impl Default for SignalProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_rms() {
        let processor = SignalProcessor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let rms = processor.compute_rms(&values);
        assert!((rms - 3.3166).abs() < 0.001);
    }

    #[test]
    fn test_empty_values() {
        let processor = SignalProcessor::new();
        assert_eq!(processor.compute_rms(&[]), 0.0);
        assert_eq!(processor.compute_peak_to_peak(&[]), 0.0);
    }
}
