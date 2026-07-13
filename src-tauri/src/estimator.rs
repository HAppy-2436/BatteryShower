//! Time-windowed average power + remaining-time estimator.
//!
//! Windows' built-in remaining-time field uses instantaneous power and is very
//! noisy. This module keeps a sliding window of recent samples and exposes
//! [`estimate_remaining`] which uses the **average** power for the calculation.

use std::collections::VecDeque;

pub struct PowerAverage {
    window_sec: i64,
    samples: VecDeque<(i64, f64)>, // (timestamp, power_watts)
}

impl PowerAverage {
    pub fn new(window_sec: i64) -> Self {
        Self {
            window_sec,
            samples: VecDeque::new(),
        }
    }

    pub fn push(&mut self, ts: i64, power: f64) {
        self.samples.push_back((ts, power));
        let cutoff = ts - self.window_sec;
        while let Some(&(front_ts, _)) = self.samples.front() {
            if front_ts < cutoff {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn avg(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let sum: f64 = self.samples.iter().map(|(_, p)| p).sum();
        Some(sum / self.samples.len() as f64)
    }
}

/// Estimate remaining (or charge-completion) seconds.
///
/// - `current_mwh`  — energy currently in the battery
/// - `full_mwh`     — design or full-charge capacity
/// - `avg_w`        — average power from [`PowerAverage`]; sign convention:
///                     discharge → positive, charge → positive (magnitude).
///
/// Returns positive seconds; 0 if inputs are not usable.
pub fn estimate_remaining_seconds(current_mwh: f64, full_mwh: f64, avg_w: f64) -> i64 {
    if full_mwh <= 0.0 || avg_w.abs() < 0.01 {
        return 0;
    }
    let current_mwh = current_mwh.max(0.0);
    let remaining_mwh = (full_mwh - current_mwh).abs();
    let wh = remaining_mwh / 1000.0;
    let hours = wh / avg_w.abs();
    (hours * 3600.0).max(0.0) as i64
}

pub fn format_remaining(seconds: i64) -> String {
    if seconds <= 0 {
        return "—".to_string();
    }
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{}h{:02}m", h, m)
    } else if m > 0 {
        format!("{}m{:02}s", m, s)
    } else {
        format!("{}s", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_keeps_window() {
        let mut a = PowerAverage::new(10);
        a.push(0, 10.0);
        a.push(5, 20.0);
        a.push(11, 30.0); // first sample should be dropped
        let avg = a.avg().unwrap();
        // After drop: 20.0 and 30.0 → avg = 25.0
        assert!((avg - 25.0).abs() < 0.01, "got {}", avg);
    }

    #[test]
    fn format_remaining_hours_minutes() {
        assert_eq!(format_remaining(3725), "1h02m");
        assert_eq!(format_remaining(125), "2m05s");
        assert_eq!(format_remaining(30), "30s");
    }

    #[test]
    fn estimate_charging_full() {
        // full=50000 mWh, currently 20000 mWh, avg 30 W
        // remaining energy = 30 Wh, time = 30 Wh / 30 W = 1 h = 3600 s
        // (Previous test asserted ~120_000 s — that was a unit mistake
        // treating 30000 mWh as 1000 Wh instead of 30 Wh.)
        let s = estimate_remaining_seconds(20000.0, 50000.0, 30.0);
        assert!(s > 3500 && s < 3700, "got {}", s);
    }
}
