//! System information types shared by telemetry and the built-in tools.

use serde::{Deserialize, Serialize};

use crate::platform::Platform;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    pub platform: Platform,
    /// e.g. "Windows 11 Pro" or "Ubuntu 24.04".
    pub name: String,
    pub version: Option<String>,
    /// CPU architecture, e.g. "x86_64" or "aarch64".
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub brand: String,
    pub logical_cores: u32,
    /// Global usage 0–100. `None` until two samples exist (the first reading
    /// after start-up cannot measure usage).
    pub usage_percent: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_bytes: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub used_bytes: u64,
}

impl MemoryInfo {
    pub fn used_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        self.used_bytes as f64 / self.total_bytes as f64 * 100.0
    }
}

/// A point-in-time reading of the machine. Contains no user data: no host
/// name, user name, process list or paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    pub os: OsInfo,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub uptime_secs: u64,
}

/// Formats bytes using binary units with one decimal, e.g. `15.6 GB`.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_in_binary_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(16 * 1024 * 1024 * 1024), "16.0 GB");
    }

    #[test]
    fn used_percent_handles_zero_total() {
        let m = MemoryInfo {
            total_bytes: 0,
            used_bytes: 0,
        };
        assert_eq!(m.used_percent(), 0.0);
    }
}
