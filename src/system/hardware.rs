use procfs::{CpuInfo, CpuTime, Current, CurrentSI, KernelStats, Meminfo};
use std::ffi::CString;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Default)]
pub struct RawCpuTicks {
    pub active: u64,
    pub total: u64,
}

impl RawCpuTicks {
    fn from_cpu_time(t: &CpuTime) -> Self {
        let idle = t.idle.saturating_add(t.iowait.unwrap_or(0));
        let active = t
            .user
            .saturating_add(t.nice)
            .saturating_add(t.system)
            .saturating_add(t.irq.unwrap_or(0))
            .saturating_add(t.softirq.unwrap_or(0))
            .saturating_add(t.steal.unwrap_or(0));
        Self {
            active,
            total: active.saturating_add(idle),
        }
    }

    fn usage_since(self, prev: Self) -> Option<f32> {
        let d_total = self.total.saturating_sub(prev.total);
        let d_active = self.active.saturating_sub(prev.active);
        if d_total == 0 {
            None
        } else {
            Some(((d_active as f64 / d_total as f64) * 100.0).clamp(0.0, 100.0) as f32)
        }
    }
}

#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    pub model: Option<String>,
    pub cores: usize,
    pub freq_ghz: Option<f32>,
    pub usage_percent: Option<f32>,
    pub per_core_percent: Vec<f32>,
    pub temp_celsius: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub swap_total_bytes: Option<u64>,
    pub swap_used_bytes: Option<u64>,
}

impl MemorySnapshot {
    pub fn usage_percent(&self) -> Option<f32> {
        let total = self.total_bytes?;
        let used = self.used_bytes?;
        if total == 0 {
            None
        } else {
            Some(((used as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

impl DiskInfo {
    pub fn usage_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            ((self.used_bytes as f64 / self.total_bytes as f64) * 100.0).clamp(0.0, 100.0) as f32
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub is_discrete: Option<bool>,
    pub busy_percent: Option<f32>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub cpu: CpuSnapshot,
    pub memory: MemorySnapshot,
    pub disks: Vec<DiskInfo>,
    pub gpus: Vec<GpuInfo>,
}

pub fn read_cpu_metadata() -> (Option<String>, usize) {
    let Ok(info) = CpuInfo::current() else {
        return (None, 0);
    };
    let cores = info.num_cores();
    let model = info
        .get_info(0)
        .and_then(|map| map.get("model name").copied())
        .map(clean_cpu_model);
    (model, cores)
}

fn clean_cpu_model(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn read_raw_cpu_ticks() -> (Option<RawCpuTicks>, Vec<RawCpuTicks>) {
    let Ok(stats) = KernelStats::current() else {
        return (None, Vec::new());
    };
    let total = RawCpuTicks::from_cpu_time(&stats.total);
    let cores = stats
        .cpu_time
        .iter()
        .map(RawCpuTicks::from_cpu_time)
        .collect();
    (Some(total), cores)
}

pub fn collect_hardware_info(
    cpu_model: &Option<String>,
    cpu_cores: usize,
    prev_total: &mut Option<RawCpuTicks>,
    prev_cores: &mut Vec<RawCpuTicks>,
) -> HardwareInfo {
    let (cur_total, cur_cores) = read_raw_cpu_ticks();
    let usage_percent = match (cur_total, *prev_total) {
        (Some(cur), Some(prev)) => cur.usage_since(prev),
        _ => None,
    };
    let mut per_core_percent = Vec::new();
    for (idx, cur) in cur_cores.iter().enumerate() {
        if let Some(prev) = prev_cores.get(idx)
            && let Some(pct) = cur.usage_since(*prev)
        {
            per_core_percent.push(pct);
        }
    }
    if cur_total.is_some() {
        *prev_total = cur_total;
    }
    if !cur_cores.is_empty() {
        *prev_cores = cur_cores;
    }

    let cpu = CpuSnapshot {
        model: cpu_model.clone(),
        cores: cpu_cores,
        freq_ghz: read_cpu_freq_ghz(),
        usage_percent,
        per_core_percent,
        temp_celsius: read_cpu_temperature(),
    };

    let memory = collect_memory();
    let disks = collect_disks();
    let gpus = collect_gpus();

    HardwareInfo {
        cpu,
        memory,
        disks,
        gpus,
    }
}

fn collect_memory() -> MemorySnapshot {
    let Ok(mem) = Meminfo::current() else {
        return MemorySnapshot {
            total_bytes: None,
            used_bytes: None,
            available_bytes: None,
            swap_total_bytes: None,
            swap_used_bytes: None,
        };
    };

    let total = mem.mem_total;
    let available = mem.mem_available.unwrap_or_else(|| {
        mem.mem_free
            .saturating_add(mem.buffers)
            .saturating_add(mem.cached)
    });
    let used = total.saturating_sub(available);
    let swap_used = mem.swap_total.saturating_sub(mem.swap_free);

    MemorySnapshot {
        total_bytes: Some(total),
        used_bytes: Some(used),
        available_bytes: Some(available),
        swap_total_bytes: (mem.swap_total > 0).then_some(mem.swap_total),
        swap_used_bytes: (mem.swap_total > 0).then_some(swap_used),
    }
}

fn read_cpu_temperature() -> Option<f32> {
    // Prefer known CPU hwmon sensor chips first
    let preferred = ["coretemp", "k10temp", "zenpower", "cpu_thermal", "acpitz"];
    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        let mut candidates: Vec<(usize, PathBufWrapper)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = fs::read_to_string(path.join("name"))
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            let priority = preferred
                .iter()
                .position(|p| name.contains(p))
                .unwrap_or(preferred.len());
            candidates.push((priority, PathBufWrapper(path)));
        }
        candidates.sort_by_key(|(prio, _)| *prio);
        for (prio, wrapper) in candidates {
            if prio == preferred.len() {
                continue;
            }
            if let Some(temp) = read_hwmon_temp(&wrapper.0) {
                return Some(temp);
            }
        }
    }

    // Fallback to thermal_zone*
    if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = entry.file_name();
            if !fname.to_string_lossy().starts_with("thermal_zone") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(path.join("temp"))
                && let Ok(milli) = raw.trim().parse::<f32>()
                && milli > 0.0
                && milli < 150_000.0
            {
                return Some(milli / 1000.0);
            }
        }
    }

    None
}

struct PathBufWrapper(std::path::PathBuf);

fn read_hwmon_temp(dir: &Path) -> Option<f32> {
    for idx in 1..=8 {
        let input = dir.join(format!("temp{idx}_input"));
        if let Ok(raw) = fs::read_to_string(input)
            && let Ok(milli) = raw.trim().parse::<f32>()
            && milli > 0.0
            && milli < 150_000.0
        {
            return Some(milli / 1000.0);
        }
    }
    None
}

fn collect_disks() -> Vec<DiskInfo> {
    let mut disks = Vec::new();
    let Ok(mounts) = fs::read_to_string("/proc/mounts") else {
        return disks;
    };

    let mut seen_mounts = std::collections::HashSet::new();
    let allowed_fs = [
        "ext4", "ext3", "ext2", "btrfs", "xfs", "zfs", "f2fs", "ntfs", "ntfs3", "vfat", "exfat",
    ];

    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let Some(source) = parts.next() else { continue };
        let Some(mount_point) = parts.next() else {
            continue;
        };
        let Some(fs_type) = parts.next() else {
            continue;
        };

        let is_root = mount_point == "/";
        let is_real_device = source.starts_with("/dev/");
        if !is_root && (!is_real_device || !allowed_fs.contains(&fs_type)) {
            continue;
        }
        if mount_point.starts_with("/boot") && disks.iter().any(|d: &DiskInfo| d.mount_point == "/")
        {
            // Keep EFI/boot secondary unless it's distinct and user has few disks; skip snap/loop noise
        }
        if source.contains("loop") || !seen_mounts.insert(mount_point.to_string()) {
            continue;
        }

        if let Some((total, used, avail)) = statvfs_bytes(mount_point) {
            if total == 0 {
                continue;
            }
            disks.push(DiskInfo {
                mount_point: mount_point.to_string(),
                fs_type: fs_type.to_string(),
                total_bytes: total,
                used_bytes: used,
                available_bytes: avail,
            });
        }
    }

    // Put '/' first, then largest disks
    disks.sort_by(|a, b| {
        (a.mount_point != "/")
            .cmp(&(b.mount_point != "/"))
            .then_with(|| b.total_bytes.cmp(&a.total_bytes))
    });
    disks.truncate(3);
    disks
}

fn statvfs_bytes(mount_point: &str) -> Option<(u64, u64, u64)> {
    let c_path = CString::new(mount_point).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return None;
    }
    let frsize = stat.f_frsize as u64;
    let total = (stat.f_blocks as u64).saturating_mul(frsize);
    let avail = (stat.f_bavail as u64).saturating_mul(frsize);
    let free = (stat.f_bfree as u64).saturating_mul(frsize);
    let used = total.saturating_sub(free);
    Some((total, used, avail))
}

fn collect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return gpus;
    };

    let mut cards: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("card") && !n.contains('-'))
        })
        .collect();
    cards.sort();

    for card in cards {
        let dev = card.join("device");
        if !dev.exists() {
            continue;
        }
        let uevent = fs::read_to_string(dev.join("uevent")).unwrap_or_default();
        let driver = extract_uevent_field(&uevent, "DRIVER");
        let pci_id = extract_uevent_field(&uevent, "PCI_ID");
        let vendor = fs::read_to_string(dev.join("vendor"))
            .ok()
            .map(|s| s.trim().to_ascii_lowercase());
        let device_id = fs::read_to_string(dev.join("device"))
            .ok()
            .map(|s| s.trim().to_ascii_lowercase());

        let busy_percent = fs::read_to_string(dev.join("gpu_busy_percent"))
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .map(|v| v.clamp(0.0, 100.0));

        let vram_used_bytes = fs::read_to_string(dev.join("mem_info_vram_used"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok());

        let vram_total_bytes = fs::read_to_string(dev.join("mem_info_vram_total"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok());

        let vendor_name = match vendor.as_deref() {
            Some("0x10de") => "NVIDIA",
            Some("0x1002") => "AMD Radeon",
            Some("0x8086") => "Intel Graphics",
            Some("0x1af4") => "VirtIO GPU",
            _ => "GPU",
        };

        let mut name = read_nvidia_model().filter(|_| vendor.as_deref() == Some("0x10de"));
        if name.is_none() {
            name = Some(match (&pci_id, &device_id) {
                (Some(pid), _) => format!("{vendor_name} [{pid}]"),
                (None, Some(did)) => format!("{vendor_name} [{did}]"),
                (None, None) => vendor_name.to_string(),
            });
        }

        let is_discrete = match vendor.as_deref() {
            Some("0x10de") => Some(true),
            Some("0x8086") => Some(false),
            _ => fs::read_to_string(dev.join("boot_vga"))
                .ok()
                .map(|s| s.trim() == "0"),
        };

        gpus.push(GpuInfo {
            name,
            driver,
            is_discrete,
            busy_percent,
            vram_used_bytes,
            vram_total_bytes,
        });
    }

    gpus
}

fn extract_uevent_field(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            let val = v.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn read_nvidia_model() -> Option<String> {
    let entries = fs::read_dir("/proc/driver/nvidia/gpus").ok()?;
    for entry in entries.flatten() {
        let info_path = entry.path().join("information");
        if let Ok(content) = fs::read_to_string(info_path) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once(':')
                    && k.trim().eq_ignore_ascii_case("Model")
                {
                    let model = v.trim();
                    if !model.is_empty() {
                        return Some(model.to_string());
                    }
                }
            }
        }
    }
    None
}

fn read_cpu_freq_ghz() -> Option<f32> {
    if let Ok(raw) = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
        && let Ok(khz) = raw.trim().parse::<f32>()
        && khz > 0.0
    {
        return Some(khz / 1_000_000.0);
    }
    if let Ok(info) = fs::read_to_string("/proc/cpuinfo") {
        for line in info.lines() {
            if let Some((k, v)) = line.split_once(':')
                && k.trim() == "cpu MHz"
                && let Ok(mhz) = v.trim().parse::<f32>()
                && mhz > 0.0
            {
                return Some(mhz / 1000.0);
            }
        }
    }
    None
}
