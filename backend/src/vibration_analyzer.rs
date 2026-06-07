use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{self, Duration};
use tracing::{info, error, warn, debug};
use rustfft::{FftPlanner, num_complex::Complex};
use chrono::Utc;

use crate::config::Config;
use crate::models::{
    SensorData, AnalyzedMetrics, TimeDomainFeatures, 
    FrequencyDomainFeatures, OperatingCondition,
};
use crate::ethercat_driver::EthercatDriver;

const CHANNEL_CAPACITY: usize = 2048;
const BATCH_PROCESS_SIZE: usize = 32;

pub struct VibrationAnalyzer {
    config: Config,
    fft_planner: std::sync::Mutex<FftPlanner<f64>>,
    severity_thresholds: (f64, f64),
}

impl VibrationAnalyzer {
    pub fn new(config: &Config) -> (Arc<Self>, mpsc::Sender<SensorData>, mpsc::Receiver<AnalyzedMetrics>) {
        let (in_tx, in_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (out_tx, out_rx) = mpsc::channel(CHANNEL_CAPACITY);

        let analyzer = Arc::new(Self {
            config: config.clone(),
            fft_planner: std::sync::Mutex::new(FftPlanner::new()),
            severity_thresholds: (
                config.monitoring.vibration_warning,
                config.monitoring.vibration_alarm,
            ),
        });

        let analyzer_clone = analyzer.clone();
        tokio::spawn(async move {
            if let Err(e) = analyzer_clone.process_loop(in_rx, out_tx).await {
                error!("VibrationAnalyzer process loop error: {}", e);
            }
        });

        (analyzer, in_tx, out_rx)
    }

    async fn process_loop(
        &self,
        mut in_rx: mpsc::Receiver<SensorData>,
        out_tx: mpsc::Sender<AnalyzedMetrics>,
    ) -> anyhow::Result<()> {
        let mut batch = Vec::with_capacity(BATCH_PROCESS_SIZE);
        let mut flush_interval = time::interval(Duration::from_millis(10));
        let mut processed_count = 0u64;
        let mut stats_interval = time::interval(Duration::from_secs(30));

        info!("VibrationAnalyzer: Processing loop started");

        loop {
            tokio::select! {
                Some(data) = in_rx.recv() => {
                    batch.push(data);
                    if batch.len() >= BATCH_PROCESS_SIZE {
                        self.process_batch(&mut batch, &out_tx).await?;
                        processed_count += batch.len() as u64;
                        batch.clear();
                    }
                }
                _ = flush_interval.tick() => {
                    if !batch.is_empty() {
                        self.process_batch(&mut batch, &out_tx).await?;
                        processed_count += batch.len() as u64;
                        batch.clear();
                    }
                }
                _ = stats_interval.tick() => {
                    debug!("VibrationAnalyzer: Processed {} metrics in 30s", processed_count);
                    processed_count = 0;
                }
            }
        }
    }

    async fn process_batch(
        &self,
        batch: &mut Vec<SensorData>,
        out_tx: &mpsc::Sender<AnalyzedMetrics>,
    ) -> anyhow::Result<()> {
        for sensor_data in batch.drain(..) {
            let analyzed = self.analyze(sensor_data);
            if let Err(e) = out_tx.send(analyzed).await {
                warn!("VibrationAnalyzer: Failed to send analyzed metrics: {}", e);
            }
        }
        Ok(())
    }

    pub fn analyze(&self, sensor_data: SensorData) -> AnalyzedMetrics {
        let timestamp = Utc::now();
        let condition = OperatingCondition::from_rpm(sensor_data.rpm);

        let time_domain = EthercatDriver::extract_time_domain_features(&sensor_data);

        let frequency_domain = self.compute_frequency_features(&sensor_data.vibration);

        let vibration_severity = self.calculate_severity(&time_domain.rms, condition);

        AnalyzedMetrics {
            timestamp,
            machine_id: sensor_data.machine_id,
            spindle_id: sensor_data.spindle_id,
            vibration: sensor_data.vibration,
            temperature: sensor_data.temperature,
            displacement: sensor_data.displacement,
            rpm: sensor_data.rpm,
            time_domain,
            frequency_domain,
            vibration_severity,
            condition,
        }
    }

    fn compute_frequency_features(&self, vibration: &[f64]) -> FrequencyDomainFeatures {
        let mut spectrum = Vec::new();
        let mut dominant_frequencies = Vec::new();
        let mut spectral_centroids = Vec::new();
        let mut spectral_energies = Vec::new();

        for sensor_data in vibration.chunks(128) {
            let spec = self.compute_fft(sensor_data);
            spectrum.push(spec.clone());

            let (dominant_idx, dominant_val) = spec.iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or((0, &0.0));
            dominant_frequencies.push(dominant_idx as f64 * 10.0 / 128.0);

            let (centroid, energy) = self.compute_spectral_metrics(&spec);
            spectral_centroids.push(centroid);
            spectral_energies.push(energy);
        }

        FrequencyDomainFeatures {
            spectrum,
            dominant_frequencies,
            spectral_centroid: spectral_centroids,
            spectral_energy: spectral_energies,
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

    fn compute_spectral_metrics(&self, spectrum: &[f64]) -> (f64, f64) {
        if spectrum.is_empty() {
            return (0.0, 0.0);
        }

        let total_energy: f64 = spectrum.iter().map(|&v| v * v).sum();

        let mut weighted_sum = 0.0;
        let mut sum = 0.0;
        for (i, &val) in spectrum.iter().enumerate() {
            weighted_sum += i as f64 * val;
            sum += val;
        }

        let centroid = if sum > 0.0 { weighted_sum / sum } else { 0.0 };
        (centroid, total_energy)
    }

    fn calculate_severity(&self, rms_values: &[f64], condition: OperatingCondition) -> Vec<f64> {
        let threshold_factor = match condition {
            OperatingCondition::LowSpeed => 0.7,
            OperatingCondition::HighSpeed => 1.3,
            _ => 1.0,
        };

        rms_values.iter()
            .map(|&rms| rms / threshold_factor)
            .collect()
    }

    pub fn classify_severity(&self, value: f64) -> u8 {
        if value >= self.severity_thresholds.1 {
            2
        } else if value >= self.severity_thresholds.0 {
            1
        } else {
            0
        }
    }
}
