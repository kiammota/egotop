pub mod hardware;
pub mod os;
pub mod session;
pub mod uptime;

use std::collections::VecDeque;

pub use hardware::{CpuSnapshot, DiskInfo, GpuInfo, HardwareInfo, MemorySnapshot};
pub use os::OsInfo;
pub use session::SessionInfo;
pub use uptime::ActivityInfo;

const HISTORY_CAPACITY: usize = 60;

#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub os: OsInfo,
    pub hardware: HardwareInfo,
    pub session: SessionInfo,
    pub activity: ActivityInfo,
    pub cpu_history: VecDeque<u64>,
    pub mem_history: VecDeque<u64>,
}

pub struct SystemCollector {
    static_os: OsInfo,
    static_cpu_model: Option<String>,
    static_cpu_cores: usize,
    prev_cpu_total: Option<hardware::RawCpuTicks>,
    prev_cpu_cores: Vec<hardware::RawCpuTicks>,
    cpu_history: VecDeque<u64>,
    mem_history: VecDeque<u64>,
}

impl SystemCollector {
    pub fn new() -> Self {
        let static_os = os::collect_static_os_info();
        let (static_cpu_model, static_cpu_cores) = hardware::read_cpu_metadata();
        let (prev_cpu_total, prev_cpu_cores) = hardware::read_raw_cpu_ticks();

        Self {
            static_os,
            static_cpu_model,
            static_cpu_cores,
            prev_cpu_total,
            prev_cpu_cores,
            cpu_history: VecDeque::with_capacity(HISTORY_CAPACITY),
            mem_history: VecDeque::with_capacity(HISTORY_CAPACITY),
        }
    }

    pub fn collect(&mut self, terminal_cols: u16, terminal_rows: u16) -> SystemSnapshot {
        let hardware = hardware::collect_hardware_info(
            &self.static_cpu_model,
            self.static_cpu_cores,
            &mut self.prev_cpu_total,
            &mut self.prev_cpu_cores,
        );
        let session = session::collect_session_info(terminal_cols, terminal_rows);
        let activity = uptime::collect_activity_info();

        if let Some(cpu_pct) = hardware.cpu.usage_percent {
            if self.cpu_history.len() >= HISTORY_CAPACITY {
                self.cpu_history.pop_front();
            }
            self.cpu_history.push_back(cpu_pct.round().clamp(0.0, 100.0) as u64);
        }

        if let Some(mem_pct) = hardware.memory.usage_percent() {
            if self.mem_history.len() >= HISTORY_CAPACITY {
                self.mem_history.pop_front();
            }
            self.mem_history.push_back(mem_pct.round().clamp(0.0, 100.0) as u64);
        }

        SystemSnapshot {
            os: self.static_os.clone(),
            hardware,
            session,
            activity,
            cpu_history: self.cpu_history.clone(),
            mem_history: self.mem_history.clone(),
        }
    }
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;

    let b = bytes as f64;
    if b >= TIB {
        format!("{:.2} TiB", b / TIB)
    } else if b >= GIB {
        format!("{:.2} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.0} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}
