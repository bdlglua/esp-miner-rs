//! Shared snapshots only: no mutex is held across I/O or an await.
use axeos_core::{Config, History, Info, LogEntry, Logs, Sample, Telemetry};
use core::{cell::RefCell, fmt::Write};
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use embassy_time::Instant;
use heapless::String;

pub struct Runtime {
    pub active: Config,
    pub config: Config,
    pub telemetry: Telemetry,
    pub history: History,
    pub logs: Logs,
    pub log_sequence: u64,
    pub streaming_clients: u8,
    pub ip: String<16>,
    pub wifi_connected: bool,
    pub storage_available: bool,
    pub restart_required: bool,
    pub mining_started: Option<u64>,
    pub interval_started: u64,
    pub interval_nonces: u64,
    pub hash_samples: heapless::Vec<(u64, u64), 12>,
}
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<Runtime>>> =
    Mutex::new(RefCell::new(None));
pub fn initialize(config: Config, storage_available: bool) {
    STATE.lock(|s| {
        *s.borrow_mut() = Some(Runtime {
            active: config.clone(),
            config,
            telemetry: Telemetry::default(),
            history: History::new(),
            logs: Logs::new(),
            log_sequence: 0,
            streaming_clients: 0,
            ip: String::new(),
            wifi_connected: false,
            storage_available,
            restart_required: false,
            mining_started: None,
            interval_started: 0,
            interval_nonces: 0,
            hash_samples: heapless::Vec::new(),
        })
    });
    log("info", format_args!("AxeOS Rust démarré"));
}
pub fn with<R>(f: impl FnOnce(&mut Runtime) -> R) -> R {
    STATE.lock(|s| {
        f(s.borrow_mut()
            .as_mut()
            .expect("state initialized before tasks"))
    })
}
pub fn log(level: &str, args: core::fmt::Arguments<'_>) {
    let mut message = String::new();
    let _ = message.write_fmt(args);
    let entry = LogEntry {
        seconds: Instant::now().as_secs(),
        level: String::try_from(level).unwrap_or_default(),
        message,
    };
    with(|s| {
        if s.logs.is_full() {
            s.logs.remove(0);
        }
        let _ = s.logs.push(entry);
        s.log_sequence = s.log_sequence.wrapping_add(1);
    });
}
pub fn fault(message: &str) {
    with(|s| {
        s.telemetry.fault = true;
        s.telemetry.power_ready = false;
        s.telemetry.hash_rate = None;
    });
    log("error", format_args!("{}", message));
}
pub fn should_pause() -> bool {
    with(|s| s.telemetry.mining_paused || s.telemetry.fault)
}
pub fn mining_started(expected: f32) {
    with(|s| {
        s.telemetry.expected_hashrate = Some(expected);
        s.telemetry.frequency = Some(s.active.frequency);
        s.mining_started = Some(Instant::now().as_secs());
        s.hash_samples.clear();
        s.interval_nonces = 0;
        s.interval_started = Instant::now().as_secs();
    });
}
pub fn nonce(valid: bool, difficulty: f64) {
    with(|s| {
        if valid {
            s.telemetry.nonces_valid += 1;
            s.interval_nonces += 1;
            if difficulty.is_finite() {
                s.telemetry.best_diff = s.telemetry.best_diff.max(difficulty);
            }
        } else {
            s.telemetry.nonces_invalid += 1;
        }
    });
}
pub fn sample() {
    let now = Instant::now().as_secs();
    with(|s| {
        if now.saturating_sub(s.interval_started) >= 5 {
            if s.hash_samples.is_full() {
                s.hash_samples.remove(0);
            }
            let _ = s
                .hash_samples
                .push((now.saturating_sub(s.interval_started), s.interval_nonces));
            s.interval_started = now;
            s.interval_nonces = 0;
            let seconds: u64 = s.hash_samples.iter().map(|x| x.0).sum();
            let nonces: u64 = s.hash_samples.iter().map(|x| x.1).sum();
            s.telemetry.hash_rate = if s.mining_started.is_some()
                && !s.telemetry.mining_paused
                && !s.telemetry.fault
                && seconds > 0
            {
                Some(
                    nonces as f64 * crate::board::ASIC_DIFFICULTY as f64 * 4.294967296
                        / seconds as f64,
                )
            } else {
                None
            };
        }
        if s.history.is_full() {
            s.history.remove(0);
        }
        let _ = s.history.push(Sample {
            timestamp: now,
            hash_rate: s.telemetry.hash_rate,
            temp: s.telemetry.temp,
            power: s.telemetry.power,
        });
    });
}
pub fn info() -> Info<'static> {
    with(|s| Info {
        device_model: crate::board::DEVICE_MODEL,
        asic_model: crate::board::ASIC_MODEL,
        board_version: crate::board::BOARD_VERSION,
        version: env!("CARGO_PKG_VERSION"),
        axe_os_version: "rust-0.1.0",
        uptime_seconds: Instant::now().as_secs(),
        free_heap: esp_alloc::HEAP.free(),
        ipv4: s.ip.clone(),
        wifi_connected: s.wifi_connected,
        storage_available: s.storage_available,
        restart_required: s.restart_required,
        voltage_min: crate::board::VOLTAGE_MIN_MV,
        voltage_max: crate::board::VOLTAGE_MAX_MV,
        settings: s.config.public(),
        telemetry: s.telemetry.clone(),
    })
}
