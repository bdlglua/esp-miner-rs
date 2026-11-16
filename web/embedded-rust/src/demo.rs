//! Browser-only virtual miner. Compiled only with `--features demo`.
//! No socket, hardware command, external pool call or persistent secret storage is used.
use axeos_core::{Config, History, Info, LogEntry, Logs, Patch, Sample, Telemetry};
use serde_json::{json, Value};
use std::cell::RefCell;

thread_local! {static DEMO:RefCell<Demo>=RefCell::new(Demo::new());}

pub fn request(method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    DEMO.with(|s| s.borrow_mut().handle(method, path, body))
}
struct Demo {
    config: Config,
    active: Config,
    telemetry: Telemetry,
    history: History,
    logs: Logs,
    seconds: u64,
    ticks: u64,
    restart_required: bool,
}
impl Demo {
    fn new() -> Self {
        let mut config = Config::defaults("WiFi Démonstration", "demo-only", 525, 1150);
        config.hostname = "bitaxe-demo".try_into().unwrap();
        config.pool_host = "pool.example".try_into().unwrap();
        config.pool_user = "demonstration.gamma601".try_into().unwrap();
        let mut this = Self {
            active: config.clone(),
            config,
            telemetry: Telemetry {
                temp: Some(57.4),
                vr_temp: Some(44.2),
                power: Some(17.3),
                voltage: Some(5050.0),
                current: Some(15043.0),
                core_voltage_actual: Some(1150.0),
                fan_percent: 70,
                fan_rpm: Some(3960),
                hash_rate: Some(1068.0),
                expected_hashrate: Some(1071.0),
                shares_accepted: 142,
                shares_rejected: 1,
                nonces_valid: 3508,
                nonces_invalid: 2,
                best_diff: 1284205.6,
                pool_connected: true,
                mining_paused: false,
                power_ready: true,
                fault: false,
                pool_difficulty: 1024.0,
                frequency: Some(525),
            },
            history: History::new(),
            logs: Logs::new(),
            seconds: 3600,
            ticks: 0,
            restart_required: false,
        };
        for i in 0..60 {
            let _ = this.history.push(Sample {
                timestamp: 3305 + i * 5,
                hash_rate: Some(
                    1060.0 + (i as f64 * 0.32).sin() * 45.0 + (i as f64 * 0.89).cos() * 18.0,
                ),
                temp: Some(56.5 + (i as f32 * 0.16).sin()),
                power: Some(17.3 + (i as f32 * 0.25).cos() * 0.4),
            });
        }
        this.log("info", "Démonstration Rust/WASM : aucune carte connectée");
        this.log("info", "Pool virtuel connecté, statistiques simulées");
        this
    }
    fn log(&mut self, level: &str, message: &str) {
        if self.logs.is_full() {
            self.logs.remove(0);
        }
        let _ = self.logs.push(LogEntry {
            seconds: self.seconds,
            level: level.try_into().unwrap(),
            message: message.try_into().unwrap(),
        });
    }
    fn tick(&mut self) {
        self.seconds += 3;
        self.ticks += 1;
        let t = self.ticks as f64;
        let active_frequency = self.active.frequency as f64;
        self.telemetry.expected_hashrate = Some((active_frequency * 2.04) as f32);
        self.telemetry.frequency = Some(self.active.frequency);
        self.telemetry.core_voltage_actual = Some(self.active.core_voltage as f32);
        if self.telemetry.mining_paused {
            self.telemetry.hash_rate = None;
            self.telemetry.temp = Some((self.telemetry.temp.unwrap_or(40.0) - 0.3).max(38.0));
            self.telemetry.power = Some(3.2);
        } else {
            self.telemetry.hash_rate =
                Some(active_frequency * 2.04 + (t * 0.34).sin() * 29.0 + (t * 0.8).cos() * 12.0);
            self.telemetry.temp = Some(56.7 + (t as f32 * 0.14).sin() * 1.4);
            self.telemetry.power =
                Some((active_frequency / 525.0 * 17.3) as f32 + (t as f32 * 0.22).sin() * 0.35);
            self.telemetry.nonces_valid += 3;
            if self.ticks.is_multiple_of(4) {
                self.telemetry.shares_accepted += 1;
            }
        }
        self.telemetry.fan_percent = if self.config.auto_fan {
            let temp = self.telemetry.temp.unwrap_or(57.0);
            (30.0 + (temp - 40.0).max(0.0) * 70.0 / 30.0).clamp(30.0, 100.0) as u8
        } else {
            self.config.fan_percent
        };
        self.telemetry.fan_rpm = Some(self.telemetry.fan_percent as u16 * 55 + 120);
        self.telemetry.current = Some(
            self.telemetry.power.unwrap_or(0.0) / (self.active.core_voltage as f32 / 1000.0)
                * 1000.0,
        );
        if self.history.is_full() {
            self.history.remove(0);
        }
        let _ = self.history.push(Sample {
            timestamp: self.seconds,
            hash_rate: self.telemetry.hash_rate,
            temp: self.telemetry.temp,
            power: self.telemetry.power,
        });
    }
    fn info(&self) -> Result<Value, String> {
        let info = Info {
            device_model: "Gamma",
            asic_model: "BM1370",
            board_version: "601",
            version: "0.1.0-démo",
            axe_os_version: "rust-démo",
            uptime_seconds: self.seconds,
            free_heap: 48128,
            ipv4: "192.0.2.60".try_into().unwrap(),
            wifi_connected: true,
            storage_available: true,
            restart_required: self.restart_required,
            voltage_min: 1000,
            voltage_max: 1250,
            settings: self.config.public(),
            telemetry: self.telemetry.clone(),
        };
        let mut value = serde_json::to_value(info).map_err(|e| e.to_string())?;
        value["demo"] = json!(true);
        Ok(value)
    }
    fn handle(&mut self, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
        match (method, path) {
            ("GET", "/api/system/info") => {
                self.tick();
                self.info()
            }
            ("GET", "/api/system/statistics") => {
                serde_json::to_value(&self.history).map_err(|e| e.to_string())
            }
            ("GET", "/api/system/events") => {
                serde_json::to_value(&self.logs).map_err(|e| e.to_string())
            }
            ("PATCH", "/api/system") => {
                let patch: Patch = serde_json::from_value(body.ok_or("Réglages manquants")?)
                    .map_err(|e| e.to_string())?;
                let next = self.config.apply(patch, 1000, 1250)?;
                let mut comparable = next.clone();
                comparable.auto_fan = self.active.auto_fan;
                comparable.fan_percent = self.active.fan_percent;
                self.restart_required = comparable != self.active;
                self.config = next;
                self.log("info", "Réglages de démonstration modifiés dans cet onglet");
                Ok(
                    json!({"message":"Réglages simulés enregistrés dans cet onglet uniquement.","restartRequired":self.restart_required}),
                )
            }
            ("POST", "/api/system/pause") => {
                self.telemetry.mining_paused = true;
                self.telemetry.hash_rate = None;
                self.log("info", "Pause du mineur virtuel");
                Ok(json!({"message":"Mineur virtuel en pause.","restartRequired":false}))
            }
            ("POST", "/api/system/resume") => {
                self.telemetry.mining_paused = false;
                self.log("info", "Reprise du mineur virtuel");
                Ok(json!({"message":"Mineur virtuel redémarré.","restartRequired":false}))
            }
            ("POST", "/api/system/restart") => {
                self.active = self.config.clone();
                self.restart_required = false;
                self.seconds = 0;
                self.ticks = 0;
                self.telemetry.shares_accepted = 0;
                self.telemetry.shares_rejected = 0;
                self.telemetry.nonces_valid = 0;
                self.telemetry.nonces_invalid = 0;
                self.telemetry.best_diff = 0.0;
                self.telemetry.mining_paused = false;
                self.history.clear();
                self.logs.clear();
                self.log(
                    "info",
                    "Redémarrage virtuel, réglages de démonstration appliqués",
                );
                Ok(
                    json!({"message":"Redémarrage simulé : les réglages sont appliqués au mineur virtuel.","restartRequired":false}),
                )
            }
            _ => Err("Route de démonstration inconnue".into()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_and_resume_change_virtual_metrics() {
        let mut d = Demo::new();
        d.handle("POST", "/api/system/pause", Some(json!({})))
            .unwrap();
        let info = d.handle("GET", "/api/system/info", None).unwrap();
        assert!(info["telemetry"]["hashRate"].is_null());
        assert_eq!(info["demo"], true);
        let accepted = info["telemetry"]["sharesAccepted"].as_u64().unwrap();
        for _ in 0..5 {
            d.handle("GET", "/api/system/info", None).unwrap();
        }
        assert_eq!(d.telemetry.shares_accepted, accepted);
        d.handle("POST", "/api/system/resume", Some(json!({})))
            .unwrap();
        d.tick();
        assert!(d.telemetry.hash_rate.is_some());
    }
    #[test]
    fn settings_apply_on_virtual_restart_and_are_validated() {
        let mut d = Demo::new();
        assert!(d
            .handle("PATCH", "/api/system", Some(json!({"coreVoltage":1500})))
            .is_err());
        let saved = d
            .handle("PATCH", "/api/system", Some(json!({"frequency":500})))
            .unwrap();
        assert_eq!(saved["restartRequired"], true);
        assert_eq!(d.active.frequency, 525);
        d.handle("POST", "/api/system/restart", Some(json!({})))
            .unwrap();
        assert_eq!(d.active.frequency, 500);
        assert!(!d.restart_required);
    }
    #[test]
    fn fan_does_not_require_restart_and_secrets_stay_private() {
        let mut d = Demo::new();
        let result = d
            .handle(
                "PATCH",
                "/api/system",
                Some(json!({"autoFan":false,"fanPercent":80})),
            )
            .unwrap();
        assert_eq!(result["restartRequired"], false);
        d.tick();
        assert_eq!(d.telemetry.fan_percent, 80);
        assert!(!d.info().unwrap().to_string().contains("demo-only"));
    }
}
