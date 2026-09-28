//! Portable [`SystemInfoProvider`] backed by the `sysinfo` crate.
//!
//! `sysinfo` uses native APIs on each OS (on Windows: `GlobalMemoryStatusEx`,
//! PDH/`GetSystemTimes`, registry reads for the OS version).
//! REQUIRES_WINDOWS_VALIDATION: values have only been verified on Linux.

use std::sync::Mutex;
use std::time::Instant;

use sershi_core::platform::Platform;
use sershi_core::ports::{PortError, SystemInfoProvider};
use sershi_core::system::{CpuInfo, MemoryInfo, OsInfo, SystemSnapshot};
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, System};

#[derive(Debug)]
struct Sampler {
    system: System,
    /// When CPU usage was last refreshed; `None` before the first sample.
    last_cpu_refresh: Option<Instant>,
    /// Whether at least two CPU samples exist (usage is meaningful).
    cpu_ready: bool,
}

/// Holds one long-lived `System` so repeated readings are cheap and CPU usage
/// can be computed as a delta between samples.
#[derive(Debug)]
pub struct SysinfoSystemInfo {
    sampler: Mutex<Sampler>,
    os: OsInfo,
    cpu_brand: String,
}

impl Default for SysinfoSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl SysinfoSystemInfo {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_cpu_all();
        let cpu_brand = system
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_owned())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "Unknown processor".to_owned());
        let os = OsInfo {
            platform: Platform::current(),
            name: System::long_os_version()
                .or_else(System::name)
                .unwrap_or_else(|| "Unknown OS".to_owned()),
            version: System::os_version(),
            arch: System::cpu_arch(),
        };
        Self {
            sampler: Mutex::new(Sampler {
                system,
                last_cpu_refresh: Some(Instant::now()),
                cpu_ready: false,
            }),
            os,
            cpu_brand,
        }
    }
}

impl SystemInfoProvider for SysinfoSystemInfo {
    fn snapshot(&self) -> Result<SystemSnapshot, PortError> {
        let mut sampler = self
            .sampler
            .lock()
            .map_err(|_| PortError::Platform("system sampler lock poisoned".to_owned()))?;

        sampler.system.refresh_memory();
        // CPU usage is a delta; sampling faster than the OS minimum yields
        // noise, so reuse the previous reading inside that window.
        let due = sampler
            .last_cpu_refresh
            .is_none_or(|t| t.elapsed() >= MINIMUM_CPU_UPDATE_INTERVAL);
        if due {
            sampler.system.refresh_cpu_usage();
            sampler.cpu_ready = sampler.last_cpu_refresh.is_some();
            sampler.last_cpu_refresh = Some(Instant::now());
        }

        let system = &sampler.system;
        Ok(SystemSnapshot {
            os: self.os.clone(),
            cpu: CpuInfo {
                brand: self.cpu_brand.clone(),
                logical_cores: u32::try_from(system.cpus().len()).unwrap_or(u32::MAX),
                usage_percent: sampler.cpu_ready.then(|| system.global_cpu_usage()),
            },
            memory: MemoryInfo {
                total_bytes: system.total_memory(),
                used_bytes: system.used_memory(),
            },
            uptime_secs: System::uptime(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration test against the real OS this test runs on.
    #[test]
    fn reads_plausible_values_from_the_host() {
        let provider = SysinfoSystemInfo::new();
        let first = provider.snapshot().unwrap();
        assert_eq!(first.os.platform, Platform::current());
        assert!(first.memory.total_bytes > 0);
        assert!(first.memory.used_bytes <= first.memory.total_bytes);
        assert!(first.cpu.logical_cores > 0);
        assert!(!first.os.arch.is_empty());

        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        let second = provider.snapshot().unwrap();
        let usage = second.cpu.usage_percent.expect("second sample has usage");
        assert!((0.0..=100.0).contains(&usage));
    }
}
