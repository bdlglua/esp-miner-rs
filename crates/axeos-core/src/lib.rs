//! Bounded configuration, telemetry and transport adapters for the official AxeOS interface.
#![no_std]
pub mod http;
pub mod official;
pub mod storage;
pub mod websocket;
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub hostname: String<32>,
    pub ssid: String<32>,
    pub wifi_password: String<64>,
    pub pool_host: String<128>,
    pub pool_port: u16,
    pub pool_user: String<64>,
    pub pool_password: String<64>,
    pub frequency: u16,
    pub core_voltage: u16,
    pub auto_fan: bool,
    pub fan_percent: u8,
}
impl Config {
    pub fn defaults(ssid: &str, password: &str, frequency: u16, voltage: u16) -> Self {
        Self {
            hostname: String::try_from("bitaxe").unwrap(),
            ssid: String::try_from(ssid).unwrap_or_default(),
            wifi_password: String::try_from(password).unwrap_or_default(),
            pool_host: String::try_from("public-pool.io").unwrap(),
            pool_port: 21496,
            // Do not mine to someone else's example wallet.
            pool_user: String::new(),
            pool_password: String::try_from("x").unwrap(),
            frequency,
            core_voltage: voltage,
            auto_fan: true,
            fan_percent: 70,
        }
    }
    pub fn validate(&self, min_mv: u16, max_mv: u16) -> Result<(), &'static str> {
        if self.hostname.is_empty()
            || !self
                .hostname
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || self.hostname.starts_with('-')
            || self.hostname.ends_with('-')
        {
            return Err("Nom de machine invalide (lettres, chiffres et tirets).");
        }
        if self.ssid.is_empty() {
            return Err("Le SSID ne peut pas être vide.");
        }
        if !self.wifi_password.is_empty() && !(8..=63).contains(&self.wifi_password.len()) {
            return Err("Le mot de passe Wi-Fi doit contenir 8 à 63 octets, ou être vide pour un réseau ouvert.");
        }
        if self.pool_host.is_empty()
            || !self
                .pool_host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        {
            return Err("Indiquez un nom DNS ou une IPv4 de pool, sans protocole ni chemin.");
        }
        if self.pool_port == 0 {
            return Err("Port de pool invalide.");
        }
        if !self.pool_user.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("Worker invalide.");
        }
        if !(100..=525).contains(&self.frequency) {
            return Err("Fréquence autorisée : 100 à 525 MHz.");
        }
        if !(min_mv..=max_mv).contains(&self.core_voltage) {
            return Err("Tension hors des limites de la carte.");
        }
        if !(30..=100).contains(&self.fan_percent) {
            return Err("Ventilation autorisée : 30 à 100 %.");
        }
        Ok(())
    }
    pub fn public(&self) -> PublicConfig {
        PublicConfig {
            hostname: self.hostname.clone(),
            ssid: self.ssid.clone(),
            pool_host: self.pool_host.clone(),
            pool_port: self.pool_port,
            pool_user: self.pool_user.clone(),
            frequency: self.frequency,
            core_voltage: self.core_voltage,
            auto_fan: self.auto_fan,
            fan_percent: self.fan_percent,
            wifi_password_set: !self.wifi_password.is_empty(),
            pool_password_set: !self.pool_password.is_empty(),
        }
    }
    pub fn apply(&self, patch: Patch, min_mv: u16, max_mv: u16) -> Result<Self, &'static str> {
        let mut next = self.clone();
        macro_rules! fields { ($($name:ident),*) => { $(if let Some(value) = patch.$name { next.$name = value; })* }; }
        fields!(
            hostname,
            ssid,
            wifi_password,
            pool_host,
            pool_port,
            pool_user,
            pool_password,
            frequency,
            core_voltage,
            auto_fan,
            fan_percent
        );
        next.validate(min_mv, max_mv)?;
        Ok(next)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicConfig {
    pub hostname: String<32>,
    pub ssid: String<32>,
    pub pool_host: String<128>,
    pub pool_port: u16,
    pub pool_user: String<64>,
    pub frequency: u16,
    pub core_voltage: u16,
    pub auto_fan: bool,
    pub fan_percent: u8,
    pub wifi_password_set: bool,
    pub pool_password_set: bool,
}
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    pub hostname: Option<String<32>>,
    pub ssid: Option<String<32>>,
    pub wifi_password: Option<String<64>>,
    pub pool_host: Option<String<128>>,
    pub pool_port: Option<u16>,
    pub pool_user: Option<String<64>>,
    pub pool_password: Option<String<64>>,
    pub frequency: Option<u16>,
    pub core_voltage: Option<u16>,
    pub auto_fan: Option<bool>,
    pub fan_percent: Option<u8>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    pub temp: Option<f32>,
    pub vr_temp: Option<f32>,
    pub power: Option<f32>,
    pub voltage: Option<f32>,
    pub current: Option<f32>,
    pub core_voltage_actual: Option<f32>,
    pub fan_percent: u8,
    pub fan_rpm: Option<u16>,
    pub hash_rate: Option<f64>,
    pub expected_hashrate: Option<f32>,
    pub shares_accepted: u64,
    pub shares_rejected: u64,
    pub nonces_valid: u64,
    pub nonces_invalid: u64,
    pub best_diff: f64,
    pub pool_connected: bool,
    pub mining_paused: bool,
    pub power_ready: bool,
    pub fault: bool,
    pub pool_difficulty: f64,
    pub frequency: Option<u16>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub timestamp: u64,
    pub hash_rate: Option<f64>,
    pub temp: Option<f32>,
    pub power: Option<f32>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub seconds: u64,
    pub level: String<8>,
    pub message: String<112>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info<'a> {
    pub device_model: &'a str,
    pub asic_model: &'a str,
    pub board_version: &'a str,
    pub version: &'a str,
    pub axe_os_version: &'a str,
    pub uptime_seconds: u64,
    pub free_heap: usize,
    pub ipv4: String<16>,
    pub wifi_connected: bool,
    pub storage_available: bool,
    pub restart_required: bool,
    pub voltage_min: u16,
    pub voltage_max: u16,
    pub settings: PublicConfig,
    pub telemetry: Telemetry,
}
pub type History = Vec<Sample, 60>;
pub type Logs = Vec<LogEntry, 32>;

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config::defaults("wifi", "password123", 525, 1150)
    }
    #[test]
    fn patch_is_atomic_and_preserves_secrets() {
        let before = config();
        let patch = serde_json::from_str(r#"{"frequency":500,"fanPercent":80}"#).unwrap();
        let next = before.apply(patch, 1000, 1250).unwrap();
        assert_eq!(next.wifi_password, before.wifi_password);
        assert_eq!(next.frequency, 500);
        assert!(before
            .apply(
                serde_json::from_str(r#"{"frequency":500,"coreVoltage":1400}"#).unwrap(),
                1000,
                1250
            )
            .is_err());
        assert_eq!(before.frequency, 525);
    }
    #[test]
    fn public_settings_never_serialize_passwords() {
        let mut cfg = config();
        cfg.pool_password = String::try_from("secret-pool").unwrap();
        let json = serde_json::to_string(&cfg.public()).unwrap();
        assert!(!json.contains("password123"));
        assert!(!json.contains("secret-pool"));
    }
    #[test]
    fn bounds_and_unknown_fields_are_rejected() {
        assert!(serde_json::from_str::<Patch>(r#"{"overclock":true}"#).is_err());
        for json in [
            r#"{"frequency":526}"#,
            r#"{"coreVoltage":999}"#,
            r#"{"fanPercent":0}"#,
            r#"{"poolHost":"http://pool.io"}"#,
            r#"{"hostname":"bad name"}"#,
        ] {
            assert!(config()
                .apply(serde_json::from_str(json).unwrap(), 1000, 1250)
                .is_err());
        }
        let long = std::format!(r#"{{"ssid":"{}"}}"#, "a".repeat(33));
        assert!(serde_json::from_str::<Patch>(&long).is_err());
    }
}
