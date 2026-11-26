use chrono::{DateTime, Duration, Local};
use procfs::{Current, CurrentSI, KernelStats, LoadAverage, Uptime};
use std::fs;

#[derive(Debug, Clone)]
pub struct ActivityInfo {
    pub uptime_secs: Option<u64>,
    pub boot_time: Option<DateTime<Local>>,
    pub load_1m: Option<f32>,
    pub load_5m: Option<f32>,
    pub load_15m: Option<f32>,
    pub process_count: Option<usize>,
    pub thread_count: Option<u32>,
    pub runnable_threads: Option<u32>,
}

pub fn collect_activity_info() -> ActivityInfo {
    let uptime_secs = Uptime::current().ok().map(|u| u.uptime.max(0.0) as u64);

    let boot_time = KernelStats::current()
        .ok()
        .and_then(|ks| DateTime::from_timestamp(ks.btime as i64, 0))
        .map(|dt| dt.with_timezone(&Local))
        .or_else(|| {
            uptime_secs.map(|secs| Local::now() - Duration::seconds(secs as i64))
        });

    let load = LoadAverage::current().ok();
    let (load_1m, load_5m, load_15m, runnable_threads, thread_count) = match load {
        Some(l) => (
            Some(l.one),
            Some(l.five),
            Some(l.fifteen),
            Some(l.cur),
            Some(l.max),
        ),
        None => (None, None, None, None, None),
    };

    let process_count = count_proc_pids();

    ActivityInfo {
        uptime_secs,
        boot_time,
        load_1m,
        load_5m,
        load_15m,
        process_count,
        thread_count,
        runnable_threads,
    }
}

fn count_proc_pids() -> Option<usize> {
    let entries = fs::read_dir("/proc").ok()?;
    let count = entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()))
        })
        .count();
    Some(count)
}

pub fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    let rem_secs = secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m {rem_secs}s")
    } else if mins > 0 {
        format!("{mins}m {rem_secs}s")
    } else {
        format!("{rem_secs}s")
    }
}
