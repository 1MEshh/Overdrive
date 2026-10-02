use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TelemetryError {
    #[error("Frametime dataset is empty or contains insufficient valid samples (minimum 2 required)")]
    InsufficientSamples,

    #[error("Failed to parse numeric value '{value}' in CSV at line {line}: {reason}")]
    ParseFloatError {
        line: usize,
        value: String,
        reason: String,
    },

    #[error("I/O error reading telemetry data: {0}")]
    Io(String),

    #[error("Could not locate valid frametime column in CSV header")]
    HeaderNotFound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub total_frames: usize,
    pub duration_seconds: f64,
    pub avg_fps: f64,
    pub min_fps: f64,
    pub max_fps: f64,
    pub low_1pct_fps: f64,
    pub low_01pct_fps: f64,
    pub dur_weighted_1pct_fps: f64,
    pub dur_weighted_01pct_fps: f64,
    pub mean_frametime_ms: f64,
    pub median_frametime_ms: f64,
    pub min_frametime_ms: f64,
    pub max_frametime_ms: f64,
    pub std_dev_ms: f64,
    pub rmssd_jitter_ms: f64,
    pub stutter_index_pct: f64,
    pub stutter_frames_count: usize,
}

impl BenchmarkMetrics {
    pub fn to_ascii_card(&self) -> String {
        let mut out = String::new();
        out.push_str("+--------------------------------------------------------------+\n");
        out.push_str("|            OVERDRIVE FRAMETIME TELEMETRY REPORT              |\n");
        out.push_str("+--------------------------------------------------------------+\n");
        out.push_str(&format!(
            "| Total Frames Analyzed : {:<36} |\n",
            self.total_frames
        ));
        out.push_str(&format!(
            "| Benchmark Duration    : {:<33.2} s |\n",
            self.duration_seconds
        ));
        out.push_str("+--------------------------------------------------------------+\n");
        out.push_str("| FRAME RATE METRICS (FPS)                                     |\n");
        out.push_str(&format!(
            "|   Average FPS         : {:<34.2}   |\n",
            self.avg_fps
        ));
        out.push_str(&format!(
            "|   Maximum FPS         : {:<34.2}   |\n",
            self.max_fps
        ));
        out.push_str(&format!(
            "|   Minimum FPS         : {:<34.2}   |\n",
            self.min_fps
        ));
        out.push_str(&format!(
            "|   1.0% Low (P99)      : {:<34.2}   |\n",
            self.low_1pct_fps
        ));
        out.push_str(&format!(
            "|   0.1% Low (P99.9)    : {:<34.2}   |\n",
            self.low_01pct_fps
        ));
        out.push_str(&format!(
            "|   1.0% Low (Duration) : {:<34.2}   |\n",
            self.dur_weighted_1pct_fps
        ));
        out.push_str(&format!(
            "|   0.1% Low (Duration) : {:<34.2}   |\n",
            self.dur_weighted_01pct_fps
        ));
        out.push_str("+--------------------------------------------------------------+\n");
        out.push_str("| FRAME PACING & STUTTER METRICS (MS)                          |\n");
        out.push_str(&format!(
            "|   Mean Frametime      : {:<33.2} ms|\n",
            self.mean_frametime_ms
        ));
        out.push_str(&format!(
            "|   Median Frametime    : {:<33.2} ms|\n",
            self.median_frametime_ms
        ));
        out.push_str(&format!(
            "|   Standard Deviation  : {:<33.2} ms|\n",
            self.std_dev_ms
        ));
        out.push_str(&format!(
            "|   Frame Pacing Jitter : {:<33.2} ms|\n",
            self.rmssd_jitter_ms
        ));
        out.push_str(&format!(
            "|   Stutter Index       : {:<33.2} % |\n",
            self.stutter_index_pct
        ));
        out.push_str(&format!(
            "|   Stutter Frame Count : {:<36} |\n",
            self.stutter_frames_count
        ));
        out.push_str("+--------------------------------------------------------------+\n");
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn to_csv_summary(&self) -> String {
        format!(
            "total_frames,duration_s,avg_fps,max_fps,min_fps,p99_1pct_low,p999_01pct_low,dur_1pct_low,dur_01pct_low,mean_ms,median_ms,std_dev_ms,rmssd_ms,stutter_index_pct,stutter_count\n{},{:.4},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3},{:.3},{:.3},{:.3},{:.2},{}\n",
            self.total_frames,
            self.duration_seconds,
            self.avg_fps,
            self.max_fps,
            self.min_fps,
            self.low_1pct_fps,
            self.low_01pct_fps,
            self.dur_weighted_1pct_fps,
            self.dur_weighted_01pct_fps,
            self.mean_frametime_ms,
            self.median_frametime_ms,
            self.std_dev_ms,
            self.rmssd_jitter_ms,
            self.stutter_index_pct,
            self.stutter_frames_count
        )
    }
}

pub fn calculate_metrics(raw_frametimes_ms: &[f64]) -> Result<BenchmarkMetrics, TelemetryError> {
    // Filter and sanitize: must be > 0.0 and <= 10000.0 (10 seconds cutoff for extreme hangs)
    let sanitized: Vec<f64> = raw_frametimes_ms
        .iter()
        .copied()
        .filter(|&t| t.is_finite() && t > 0.0 && t <= 10000.0)
        .collect();

    let n = sanitized.len();
    if n < 2 {
        return Err(TelemetryError::InsufficientSamples);
    }

    let n_f64 = n as f64;
    let sum_duration_ms: f64 = sanitized.iter().sum();
    let duration_seconds = sum_duration_ms / 1000.0;
    let avg_fps = (n_f64 * 1000.0) / sum_duration_ms;
    let mean_frametime_ms = sum_duration_ms / n_f64;

    // Standard deviation
    let variance_sum: f64 = sanitized
        .iter()
        .map(|&t| {
            let diff = t - mean_frametime_ms;
            diff * diff
        })
        .sum();
    let std_dev_ms = (variance_sum / n_f64).sqrt();

    // Frame Pacing Jitter: Root Mean Square of Successive Differences (RMSSD)
    let mut sum_diff_sq = 0.0;
    for i in 1..n {
        let delta = sanitized[i] - sanitized[i - 1];
        sum_diff_sq += delta * delta;
    }
    let rmssd_jitter_ms = (sum_diff_sq / (n_f64 - 1.0)).sqrt();

    // Sorted frametimes ascending for percentiles
    let mut sorted = sanitized.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min_frametime_ms = sorted[0];
    let max_frametime_ms = sorted[n - 1];
    let max_fps = 1000.0 / min_frametime_ms;
    let min_fps = 1000.0 / max_frametime_ms;

    // Percentiles using linear interpolation
    let median_frametime_ms = interpolate_percentile(&sorted, 50.0);
    let p99_frametime_ms = interpolate_percentile(&sorted, 99.0);
    let p999_frametime_ms = interpolate_percentile(&sorted, 99.9);

    let low_1pct_fps = 1000.0 / p99_frametime_ms;
    let low_01pct_fps = 1000.0 / p999_frametime_ms;

    // Duration-weighted 1% Low and 0.1% Low
    // Sort descending: worst to best
    let mut desc = sorted.clone();
    desc.reverse();

    let dur_weighted_1pct_fps = calculate_duration_weighted_low(&desc, sum_duration_ms, 0.01);
    let dur_weighted_01pct_fps = calculate_duration_weighted_low(&desc, sum_duration_ms, 0.001);

    // Stutter Index: percentage of time spent in frames exceeding 1.5x median frametime
    let stutter_threshold = 1.5 * median_frametime_ms;
    let mut stutter_time_ms = 0.0;
    let mut stutter_frames_count = 0usize;

    for &t in &sanitized {
        if t > stutter_threshold {
            stutter_time_ms += t;
            stutter_frames_count += 1;
        }
    }

    let stutter_index_pct = if sum_duration_ms > 0.0 {
        (stutter_time_ms / sum_duration_ms) * 100.0
    } else {
        0.0
    };

    Ok(BenchmarkMetrics {
        total_frames: n,
        duration_seconds,
        avg_fps,
        min_fps,
        max_fps,
        low_1pct_fps,
        low_01pct_fps,
        dur_weighted_1pct_fps,
        dur_weighted_01pct_fps,
        mean_frametime_ms,
        median_frametime_ms,
        min_frametime_ms,
        max_frametime_ms,
        std_dev_ms,
        rmssd_jitter_ms,
        stutter_index_pct,
        stutter_frames_count,
    })
}

fn interpolate_percentile(sorted_asc: &[f64], pct: f64) -> f64 {
    let n = sorted_asc.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return sorted_asc[0];
    }

    let rank = (pct / 100.0) * ((n - 1) as f64);
    let lower_idx = rank.floor() as usize;
    let upper_idx = rank.ceil() as usize;

    if lower_idx == upper_idx || upper_idx >= n {
        sorted_asc[lower_idx.min(n - 1)]
    } else {
        let weight = rank - (lower_idx as f64);
        sorted_asc[lower_idx] + weight * (sorted_asc[upper_idx] - sorted_asc[lower_idx])
    }
}

fn calculate_duration_weighted_low(
    sorted_desc: &[f64],
    total_duration_ms: f64,
    pct_fraction: f64,
) -> f64 {
    let target_duration_ms = total_duration_ms * pct_fraction;
    let mut accumulated_time_ms = 0.0;
    let mut frame_count = 0usize;

    for &ft in sorted_desc {
        accumulated_time_ms += ft;
        frame_count += 1;
        if accumulated_time_ms >= target_duration_ms {
            break;
        }
    }

    if frame_count == 0 {
        return 0.0;
    }

    let avg_ft_of_worst = accumulated_time_ms / (frame_count as f64);
    if avg_ft_of_worst > 0.0 {
        1000.0 / avg_ft_of_worst
    } else {
        0.0
    }
}

pub fn parse_frametimes_csv(csv_content: &str) -> Result<Vec<f64>, TelemetryError> {
    let mut lines = csv_content.lines().enumerate();
    let mut frametimes = Vec::new();

    let mut target_col_idx: Option<usize> = None;
    let mut is_nanoseconds = false;

    // Inspect first non-empty line
    while let Some((_line_num, line)) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        let fields: Vec<&str> = trimmed.split(',').map(|s| s.trim()).collect();
        if fields.is_empty() {
            continue;
        }

        // Check if header row
        let mut found_header = false;
        for (i, &field) in fields.iter().enumerate() {
            let lower = field.to_ascii_lowercase();
            if lower.contains("frametime_ns") || lower == "ft_ns" {
                target_col_idx = Some(i);
                is_nanoseconds = true;
                found_header = true;
                break;
            } else if lower.contains("frametime")
                || lower == "frame_time"
                || lower == "ft"
                || lower == "frametime (ms)"
                || lower == "frametime_ms"
            {
                target_col_idx = Some(i);
                is_nanoseconds = false;
                found_header = true;
                break;
            }
        }

        if found_header {
            // Header identified, proceed to data rows
            break;
        }

        // If not a header row, check if the first row is purely numeric (header-less CSV)
        if let Ok(first_val) = fields[0].parse::<f64>() {
            let col = 0;
            target_col_idx = Some(col);
            if first_val > 50000.0 {
                is_nanoseconds = true;
                frametimes.push(first_val / 1_000_000.0);
            } else {
                frametimes.push(first_val);
            }
            break;
        } else {
            // Unknown header or label, skip
            continue;
        }
    }

    let col_idx = target_col_idx.unwrap_or(0);

    for (line_num, line) in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        let fields: Vec<&str> = trimmed.split(',').map(|s| s.trim()).collect();
        if col_idx < fields.len() {
            let raw_val = fields[col_idx].trim_matches('"');
            match raw_val.parse::<f64>() {
                Ok(val) => {
                    if is_nanoseconds {
                        frametimes.push(val / 1_000_000.0);
                    } else if val > 100_000.0 && frametimes.is_empty() {
                        // Heuristic detection: if first value is huge, treat as ns
                        is_nanoseconds = true;
                        frametimes.push(val / 1_000_000.0);
                    } else {
                        frametimes.push(val);
                    }
                }
                Err(e) => {
                    // Ignore malformed trailing rows (e.g. summary text at bottom of CSV)
                    if frametimes.len() < 2 {
                        return Err(TelemetryError::ParseFloatError {
                            line: line_num + 1,
                            value: raw_val.to_string(),
                            reason: e.to_string(),
                        });
                    }
                }
            }
        }
    }

    if frametimes.len() < 2 {
        return Err(TelemetryError::InsufficientSamples);
    }

    Ok(frametimes)
}
