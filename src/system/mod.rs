pub mod hardware;
pub mod os;
pub mod session;
pub mod uptime;

use std::collections::VecDeque;
use std::fs;
use std::time::Instant;

pub use hardware::{CpuSnapshot, DiskInfo, GpuInfo, HardwareInfo, MemorySnapshot};
pub use os::OsInfo;
pub use session::SessionInfo;
pub use uptime::ActivityInfo;

const HISTORY_CAPACITY: usize = 60;

#[derive(Debug, Clone, Default)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkSnapshot {
    pub interfaces: Vec<NetworkInterfaceInfo>,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
    pub total_rx_bps: u64,
    pub total_tx_bps: u64,
}

#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub os: OsInfo,
    pub hardware: HardwareInfo,
    pub session: SessionInfo,
    pub activity: ActivityInfo,
    pub network: NetworkSnapshot,
    pub cpu_history: VecDeque<u64>,
    pub mem_history: VecDeque<u64>,
    pub net_rx_history: VecDeque<u64>,
    pub net_tx_history: VecDeque<u64>,
}

pub struct SystemCollector {
    static_os: OsInfo,
    static_cpu_model: Option<String>,
    static_cpu_cores: usize,
    prev_cpu_total: Option<hardware::RawCpuTicks>,
    prev_cpu_cores: Vec<hardware::RawCpuTicks>,
    prev_net: Option<(Instant, Vec<(String, u64, u64)>)>,
    cpu_history: VecDeque<u64>,
    mem_history: VecDeque<u64>,
    net_rx_history: VecDeque<u64>,
    net_tx_history: VecDeque<u64>,
}

impl SystemCollector {
    pub fn new() -> Self {
        let static_os = os::collect_static_os_info();
        let (static_cpu_model, static_cpu_cores) = hardware::read_cpu_metadata();
        let (prev_cpu_total, prev_cpu_cores) = hardware::read_raw_cpu_ticks();
        let initial_net = read_proc_net_dev();

        Self {
            static_os,
            static_cpu_model,
            static_cpu_cores,
            prev_cpu_total,
            prev_cpu_cores,
            prev_net: Some((Instant::now(), initial_net)),
            cpu_history: VecDeque::with_capacity(HISTORY_CAPACITY),
            mem_history: VecDeque::with_capacity(HISTORY_CAPACITY),
            net_rx_history: VecDeque::with_capacity(HISTORY_CAPACITY),
            net_tx_history: VecDeque::with_capacity(HISTORY_CAPACITY),
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
        let network = self.collect_network();

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

        if self.net_rx_history.len() >= HISTORY_CAPACITY {
            self.net_rx_history.pop_front();
        }
        self.net_rx_history.push_back(network.total_rx_bps / 1024);

        if self.net_tx_history.len() >= HISTORY_CAPACITY {
            self.net_tx_history.pop_front();
        }
        self.net_tx_history.push_back(network.total_tx_bps / 1024);

        SystemSnapshot {
            os: self.static_os.clone(),
            hardware,
            session,
            activity,
            network,
            cpu_history: self.cpu_history.clone(),
            mem_history: self.mem_history.clone(),
            net_rx_history: self.net_rx_history.clone(),
            net_tx_history: self.net_tx_history.clone(),
        }
    }

    fn collect_network(&mut self) -> NetworkSnapshot {
        let now = Instant::now();
        let raw = read_proc_net_dev();
        let elapsed_secs = self
            .prev_net
            .as_ref()
            .map(|(t, _)| now.duration_since(*t).as_secs_f64().max(0.05))
            .unwrap_or(1.0);

        let mut interfaces = Vec::new();
        let mut total_rx_bytes = 0u64;
        let mut total_tx_bytes = 0u64;
        let mut total_rx_bps = 0u64;
        let mut total_tx_bps = 0u64;

        for (name, rx, tx) in &raw {
            if name == "lo" {
                continue;
            }
            let (prev_rx, prev_tx) = self
                .prev_net
                .as_ref()
                .and_then(|(_, prev_list)| {
                    prev_list
                        .iter()
                        .find(|(n, _, _)| n == name)
                        .map(|(_, prx, ptx)| (*prx, *ptx))
                })
                .unwrap_or((*rx, *tx));

            let rx_bps = ((rx.saturating_sub(prev_rx) as f64) / elapsed_secs) as u64;
            let tx_bps = ((tx.saturating_sub(prev_tx) as f64) / elapsed_secs) as u64;
            total_rx_bytes = total_rx_bytes.saturating_add(*rx);
            total_tx_bytes = total_tx_bytes.saturating_add(*tx);
            total_rx_bps = total_rx_bps.saturating_add(rx_bps);
            total_tx_bps = total_tx_bps.saturating_add(tx_bps);

            interfaces.push(NetworkInterfaceInfo {
                name: name.clone(),
                rx_bytes: *rx,
                tx_bytes: *tx,
                rx_bytes_per_sec: rx_bps,
                tx_bytes_per_sec: tx_bps,
            });
        }

        self.prev_net = Some((now, raw));
        NetworkSnapshot {
            interfaces,
            total_rx_bytes,
            total_tx_bytes,
            total_rx_bps,
            total_tx_bps,
        }
    }
}

fn read_proc_net_dev() -> Vec<(String, u64, u64)> {
    let Ok(content) = fs::read_to_string("/proc/net/dev") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in content.lines().skip(2) {
        let Some((iface, stats)) = line.split_once(':') else {
            continue;
        };
        let cols: Vec<&str> = stats.split_whitespace().collect();
        if cols.len() >= 9 {
            let rx = cols[0].parse::<u64>().unwrap_or(0);
            let tx = cols[8].parse::<u64>().unwrap_or(0);
            out.push((iface.trim().to_string(), rx, tx));
        }
    }
    out
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
