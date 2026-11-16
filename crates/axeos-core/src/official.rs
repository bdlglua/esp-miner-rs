//! ESP-Miner/AxeOS wire adapter. The Angular screens remain upstream sources.
use crate::{Config, History, Info, Patch};
use heapless::{String, Vec};
use serde::{
    ser::{SerializeMap, SerializeSeq},
    Deserialize, Serialize, Serializer,
};

#[derive(Debug, PartialEq)]
pub enum SettingsError {
    Invalid(&'static str),
    Unsupported(&'static str),
}
use SettingsError::{Invalid, Unsupported};

pub fn parse_settings(body: &[u8]) -> Result<Settings, SettingsError> {
    // The HTTP layer bounds this allocation-capable decoder to 2048 bytes.
    if body.len() > crate::http::MAX_BODY {
        return Err(Invalid("Requête trop longue."));
    }
    serde_json::from_slice(body).map_err(|_| Invalid("JSON invalide ou champ inconnu."))
}

// Angular sends booleans for several switches despite the numeric OpenAPI enum.
#[derive(Clone, Copy)]
pub enum Flag {
    Bool(bool),
    Number(u8),
}
impl<'de> Deserialize<'de> for Flag {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Flag;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("boolean or 0/1")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Flag, E> {
                Ok(Flag::Bool(v))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Flag, E> {
                u8::try_from(v).map(Flag::Number).map_err(E::custom)
            }
        }
        d.deserialize_any(Visitor)
    }
}
impl Flag {
    fn value(self) -> Result<bool, SettingsError> {
        match self {
            Self::Bool(v) => Ok(v),
            Self::Number(0) => Ok(false),
            Self::Number(1) => Ok(true),
            _ => Err(Invalid("Le commutateur doit valoir 0 ou 1.")),
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PoolUpdate {
    pub id: Option<u8>,
    #[serde(rename = "stratumURL")]
    pub url: Option<String<128>>,
    #[serde(rename = "stratumPort")]
    pub port: Option<u16>,
    #[serde(rename = "stratumUser")]
    pub user: Option<String<64>>,
    #[serde(rename = "stratumPassword")]
    pub password: Option<String<64>>,
    #[serde(rename = "stratumProtocol")]
    pub protocol: Option<String<8>>,
    #[serde(rename = "stratumTLS")]
    pub tls: Option<u8>,
    #[serde(rename = "stratumCert")]
    pub cert: Option<String<1>>,
    #[serde(rename = "stratumSuggestedDifficulty")]
    pub difficulty: Option<f64>,
    #[serde(rename = "stratumExtranonceSubscribe")]
    pub extranonce: Option<bool>,
    #[serde(rename = "stratumDecodeCoinbase")]
    pub decode: Option<bool>,
    #[serde(rename = "stratumShareWarning")]
    pub warning: Option<bool>,
    #[serde(rename = "stratumV2ChannelType")]
    pub v2_channel: Option<String<16>>,
    #[serde(rename = "stratumV2AuthorityPubkey")]
    pub v2_key: Option<String<1>>,
    #[serde(rename = "stratumV2RequireAuth")]
    pub v2_auth: Option<bool>,
}
impl PoolUpdate {
    fn check(&self) -> Result<(), SettingsError> {
        if self.id.is_some_and(|v| v != 0)
            || self.protocol.as_deref().is_some_and(|v| v != "SV1")
            || self.tls.is_some_and(|v| v != 0)
            || self.cert.as_deref().is_some_and(|v| !v.is_empty())
            || self.difficulty.is_some_and(|v| v != 0.0)
            || self.extranonce == Some(true)
            || self.decode == Some(true)
            || self.warning == Some(true)
            || self.v2_key.as_deref().is_some_and(|v| !v.is_empty())
            || self.v2_auth == Some(true)
            || self.v2_channel.as_deref().is_some_and(|v| v != "extended")
        {
            return Err(Unsupported("Un seul pool Stratum V1 TCP est disponible ; TLS, SV2 et décodage coinbase ne sont pas implémentés."));
        }
        Ok(())
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub hostname: Option<String<32>>,
    pub ssid: Option<String<32>>,
    #[serde(rename = "wifiPass", alias = "wifiPassword")]
    pub wifi_password: Option<String<64>>,
    #[serde(rename = "stratumURL", alias = "poolHost")]
    pub pool_host: Option<String<128>>,
    #[serde(rename = "stratumPort", alias = "poolPort")]
    pub pool_port: Option<u16>,
    #[serde(rename = "stratumUser", alias = "poolUser")]
    pub pool_user: Option<String<64>>,
    #[serde(rename = "stratumPassword", alias = "poolPassword")]
    pub pool_password: Option<String<64>>,
    pub frequency: Option<u16>,
    pub core_voltage: Option<u16>,
    #[serde(rename = "autofanspeed", alias = "autoFan")]
    pub auto_fan: Option<Flag>,
    #[serde(rename = "manualFanSpeed", alias = "fanPercent")]
    pub fan_percent: Option<u8>,
    pub pools: Option<Vec<PoolUpdate, 8>>,
    pub primary_pool_index: Option<u8>,
    pub secondary_pool_index: Option<u8>,
    pub use_fallback_stratum: Option<u8>,
    pub stratum_protocol: Option<String<8>>,
    pub display: Option<String<32>>,
    pub rotation: Option<u16>,
    pub invertscreen: Option<Flag>,
    pub display_timeout: Option<i16>,
    pub min_fan_speed: Option<u8>,
    pub temptarget: Option<u8>,
    #[serde(rename = "overheat_mode")]
    pub overheat_mode: Option<u8>,
    pub stats_frequency: Option<u16>,
    pub overclock_enabled: Option<Flag>,
    #[serde(rename = "useNTP")]
    pub use_ntp: Option<bool>,
    #[serde(rename = "useCustomWWW")]
    pub custom_www: Option<u8>,
}
fn bounded<const N: usize>(v: &str) -> Result<String<N>, SettingsError> {
    String::try_from(v).map_err(|_| Invalid("Champ trop long."))
}
impl Settings {
    pub fn apply(
        self,
        current: &Config,
        min: u16,
        max: u16,
        fault: bool,
    ) -> Result<Config, SettingsError> {
        // Accept unchanged fields sent by complete upstream forms, reject unsupported changes atomically.
        if self.display.as_deref().is_some_and(|v| v != "NONE")
            || self.rotation.is_some_and(|v| v != 0)
            || self
                .invertscreen
                .map(Flag::value)
                .transpose()?
                .unwrap_or(false)
            || self.display_timeout.is_some_and(|v| v != -1)
            || self.min_fan_speed.is_some_and(|v| v != 30)
            || self.temptarget.is_some_and(|v| v != 60)
            || self.stats_frequency.is_some_and(|v| v != 5)
            || self
                .overclock_enabled
                .map(Flag::value)
                .transpose()?
                .unwrap_or(false)
            || self.use_ntp == Some(true)
            || self.custom_www.is_some_and(|v| v != 0)
            || self.primary_pool_index.is_some_and(|v| v != 0)
            || self.secondary_pool_index.is_some_and(|v| v != 0)
            || self.use_fallback_stratum.is_some_and(|v| v != 0)
            || self.stratum_protocol.as_deref().is_some_and(|v| v != "SV1")
        {
            return Err(Unsupported("Fonction non implémentée : écran, PID personnalisé, NTP, pools multiples, SV2 ou interface personnalisée. La ventilation utilise une courbe thermique fixe."));
        }
        if self.overheat_mode.is_some_and(|v| v != u8::from(fault)) {
            return Err(Unsupported("La protection thermique reste active ; contrôler la carte et redémarrer après un défaut."));
        }
        let mut patch = Patch {
            hostname: self.hostname.as_deref().map(bounded).transpose()?,
            ssid: self.ssid.as_deref().map(bounded).transpose()?,
            wifi_password: self
                .wifi_password
                .as_deref()
                .filter(|v| *v != "*****")
                .map(bounded)
                .transpose()?,
            pool_host: self.pool_host.as_deref().map(bounded).transpose()?,
            pool_port: self.pool_port,
            pool_user: self.pool_user.as_deref().map(bounded).transpose()?,
            pool_password: self
                .pool_password
                .as_deref()
                .filter(|v| *v != "*****")
                .map(bounded)
                .transpose()?,
            frequency: self.frequency,
            core_voltage: self.core_voltage,
            auto_fan: self.auto_fan.map(Flag::value).transpose()?,
            fan_percent: self.fan_percent,
        };
        if let Some(pools) = self.pools {
            if pools.len() != 1
                || self.pool_host.is_some()
                || self.pool_port.is_some()
                || self.pool_user.is_some()
                || self.pool_password.is_some()
            {
                return Err(Unsupported(
                    "Envoyer un seul pool, sans champs Stratum contradictoires.",
                ));
            }
            let pool = &pools[0];
            pool.check()?;
            patch.pool_host = pool.url.as_deref().map(bounded).transpose()?;
            patch.pool_port = pool.port;
            patch.pool_user = pool.user.as_deref().map(bounded).transpose()?;
            patch.pool_password = pool
                .password
                .as_deref()
                .filter(|v| *v != "*****")
                .map(bounded)
                .transpose()?;
        }
        current.apply(patch, min, max).map_err(Invalid)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Pool<'a> {
    id: u8,
    #[serde(rename = "stratumURL")]
    url: &'a str,
    #[serde(rename = "stratumPort")]
    port: u16,
    #[serde(rename = "stratumUser")]
    user: &'a str,
    #[serde(rename = "stratumPassword")]
    password: &'a str,
    #[serde(rename = "stratumProtocol")]
    protocol: &'a str,
    #[serde(rename = "stratumTLS")]
    tls: u8,
    #[serde(rename = "stratumCert")]
    cert: &'a str,
    #[serde(rename = "stratumSuggestedDifficulty")]
    difficulty: u8,
    #[serde(rename = "stratumExtranonceSubscribe")]
    extranonce: bool,
    #[serde(rename = "stratumDecodeCoinbase")]
    decode: bool,
    #[serde(rename = "stratumShareWarning")]
    warning: bool,
    #[serde(rename = "stratumV2ChannelType")]
    v2_channel: &'a str,
    #[serde(rename = "stratumV2AuthorityPubkey")]
    v2_key: &'a str,
    #[serde(rename = "stratumV2RequireAuth")]
    v2_auth: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Partition<'a> {
    label: &'a str,
    version: &'a str,
    is_current: bool,
    is_factory: bool,
}

/// Flat names and units expected by the real AxeOS dashboard. Missing measurements
/// use upstream numeric sentinels (temperature -1, other values 0), listed in unavailableFields.
/// Unsupported features have explicit capability flags; no pool password is disclosed.
pub struct SystemInfo<'a>(pub &'a Info<'a>);
impl Serialize for SystemInfo<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let i = self.0;
        let c = &i.settings;
        let t = &i.telemetry;
        let mut m = serializer.serialize_map(None)?;
        macro_rules! f {
            ($key:literal,$v:expr) => {
                m.serialize_entry($key, &$v)?
            };
        }
        f!("ASICModel", i.asic_model);
        f!("deviceModel", i.device_model);
        f!("boardVersion", i.board_version);
        f!("version", i.version);
        f!("axeOSVersion", "official-e3cfbb3");
        f!("idfVersion", "esp-hal / Rust");
        f!("hostname", c.hostname);
        f!("ssid", c.ssid);
        f!("ipv4", i.ipv4);
        f!("ipv6", "");
        f!(
            "wifiStatus",
            if i.wifi_connected {
                "Connected"
            } else {
                "Disconnected"
            }
        );
        f!("wifiRSSI", 0);
        f!("apEnabled", 0);
        f!("useNTP", false);
        f!("isPSRAMAvailable", 0);
        f!("uptimeSeconds", i.uptime_seconds);
        f!("freeHeap", i.free_heap);
        f!("freeHeapInternal", i.free_heap);
        for key in ["freeHeapSpiram", "minFreeHeap", "maxAllocHeap", "cpuUsage"] {
            m.serialize_entry(key, &0)?;
        }
        f!("macAddr", "");
        f!("resetReason", "");
        f!("frequency", c.frequency);
        f!("actualFrequency", t.frequency.unwrap_or(0));
        f!("coreVoltage", c.core_voltage);
        f!("coreVoltageActual", t.core_voltage_actual.unwrap_or(0.0));
        f!("power", t.power.unwrap_or(0.0));
        f!("voltage", t.voltage.unwrap_or(0.0));
        f!("current", t.current.unwrap_or(0.0));
        f!("temp", t.temp.unwrap_or(-1.0));
        f!("temp2", -1);
        f!("vrTemp", t.vr_temp.unwrap_or(-1.0));
        f!("hashRate", t.hash_rate.unwrap_or(0.0));
        f!("hashRate_1m", t.hash_rate.unwrap_or(0.0));
        f!("hashRate_10m", 0);
        f!("hashRate_1h", 0);
        f!("expectedHashrate", t.expected_hashrate.unwrap_or(0.0));
        f!("bestDiff", t.best_diff);
        f!("bestSessionDiff", t.best_diff);
        f!("sharesAccepted", t.shares_accepted);
        f!("sharesRejected", t.shares_rejected);
        f!("sharesRejectedReasons", [0u8; 0]);
        let nonces = t.nonces_valid + t.nonces_invalid;
        f!(
            "errorPercentage",
            if nonces == 0 {
                0.0
            } else {
                100.0 * t.nonces_invalid as f64 / nonces as f64
            }
        );
        f!("miningPaused", t.mining_paused || t.fault);
        f!("overheat_mode", u8::from(t.fault));
        f!(
            "hardware_fault",
            if t.fault {
                "Hardware fault: inspect board before restarting"
            } else {
                ""
            }
        );
        f!("autofanspeed", u8::from(c.auto_fan));
        f!("manualFanSpeed", c.fan_percent);
        f!("fanspeed", t.fan_percent);
        f!("fanrpm", t.fan_rpm.unwrap_or(0));
        f!("fan2rpm", 0);
        f!("minFanSpeed", 30);
        f!("temptarget", 60);
        f!("statsFrequency", 5);
        f!("statsLimit", 60);
        f!("display", "NONE");
        f!("rotation", 0);
        f!("invertscreen", 0);
        f!("displayTimeout", -1);
        f!("overclockEnabled", 0);
        f!("useCustomWWW", 0);
        f!("stratumURL", c.pool_host);
        f!("stratumPort", c.pool_port);
        f!("stratumUser", c.pool_user);
        f!("stratumProtocol", "SV1");
        f!("stratumTLS", false);
        f!("stratumCert", "");
        f!("stratumV2AuthorityPubkey", "");
        f!("stratumSuggestedDifficulty", 0);
        f!("stratumExtranonceSubscribe", false);
        f!("stratumDecodeCoinbase", false);
        f!("stratumShareWarning", false);
        f!(
            "pools",
            [Pool {
                id: 0,
                url: &c.pool_host,
                port: c.pool_port,
                user: &c.pool_user,
                password: if c.pool_password_set { "*****" } else { "" },
                protocol: "SV1",
                tls: 0,
                cert: "",
                difficulty: 0,
                extranonce: false,
                decode: false,
                warning: false,
                v2_channel: "extended",
                v2_key: "",
                v2_auth: false
            }]
        );
        f!("primaryPoolIndex", 0);
        f!("secondaryPoolIndex", 0);
        f!("useFallbackStratum", 0);
        f!("isUsingFallbackStratum", 0);
        f!("poolDifficulty", t.pool_difficulty);
        f!(
            "poolConnectionInfo",
            if t.pool_connected {
                "IPv4"
            } else {
                "Disconnected"
            }
        );
        f!(
            "primaryPoolError",
            if t.pool_connected {
                ""
            } else {
                "Pool disconnected or not configured"
            }
        );
        f!("poolBanner", "");
        for key in [
            "fallbackStratumURL",
            "fallbackStratumUser",
            "fallbackStratumCert",
        ] {
            m.serialize_entry(key, "")?;
        }
        for key in [
            "fallbackStratumPort",
            "fallbackStratumSuggestedDifficulty",
            "blockHeight",
            "networkDifficulty",
            "blockFound",
            "coinbaseValueTotalSatoshis",
            "coinbaseValueUserSatoshis",
            "coinbaseOthersCount",
            "coinbaseOthersValueSatoshis",
            "smallCoreCount",
        ] {
            m.serialize_entry(key, &0)?;
        }
        for key in [
            "fallbackStratumExtranonceSubscribe",
            "fallbackStratumTLS",
            "fallbackStratumDecodeCoinbase",
            "fallbackStratumShareWarning",
        ] {
            m.serialize_entry(key, &false)?;
        }
        for key in [
            "responseTime",
            "responseTimeMin",
            "responseTimeMax",
            "maxPower",
            "nominalVoltage",
        ] {
            m.serialize_entry(key, &0)?;
        }
        f!("scriptsig", "");
        f!("blockSignals", [0u8; 0]);
        f!("coinbasePayoutStatus", "unknown");
        f!("runningPartition", "factory");
        f!(
            "partitions",
            [Partition {
                label: "factory",
                version: i.version,
                is_current: true,
                is_factory: true
            }]
        );
        f!("storageAvailable", i.storage_available);
        f!("restartRequired", i.restart_required);
        f!("capabilities", Capabilities::default());
        let mut unavailable = Vec::<&str, 32>::new();
        for field in [
            "temp2",
            "fan2rpm",
            "hashRate_10m",
            "hashRate_1h",
            "wifiRSSI",
            "cpuUsage",
            "macAddr",
            "resetReason",
            "responseTime",
            "responseTimeMin",
            "responseTimeMax",
            "minFreeHeap",
            "maxAllocHeap",
            "maxPower",
            "nominalVoltage",
            "blockHeight",
            "networkDifficulty",
            "blockFound",
            "bestDiffAllTime",
        ] {
            let _ = unavailable.push(field);
        }
        for (field, missing) in [
            ("temp", t.temp.is_none()),
            ("vrTemp", t.vr_temp.is_none()),
            ("power", t.power.is_none()),
            ("voltage", t.voltage.is_none()),
            ("current", t.current.is_none()),
            ("coreVoltageActual", t.core_voltage_actual.is_none()),
            ("fanrpm", t.fan_rpm.is_none()),
            ("hashRate", t.hash_rate.is_none()),
            ("expectedHashrate", t.expected_hashrate.is_none()),
            ("actualFrequency", t.frequency.is_none()),
        ] {
            if missing {
                let _ = unavailable.push(field);
            }
        }
        f!("unavailableFields", unavailable);
        m.end()
    }
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    ota: bool,
    swarm: bool,
    display: bool,
    wifi_scan: bool,
    ntp: bool,
    tls: bool,
    stratum_v2: bool,
    multiple_pools: bool,
    custom_fan_curve: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsicSettings<'a> {
    #[serde(rename = "ASICModel")]
    pub asic_model: &'a str,
    pub device_model: &'a str,
    pub swarm_color: &'a str,
    pub asic_count: u8,
    pub default_frequency: u16,
    pub default_voltage: u16,
    pub frequency_options: Vec<u16, 20>,
    pub voltage_options: Vec<u16, 20>,
}
impl<'a> AsicSettings<'a> {
    pub fn new(
        model: &'a str,
        device: &'a str,
        default_frequency: u16,
        default_voltage: u16,
        min: u16,
        max: u16,
    ) -> Self {
        let mut frequency_options = Vec::new();
        for n in (100..=525).step_by(25) {
            let _ = frequency_options.push(n);
        }
        if !frequency_options.contains(&default_frequency) {
            let _ = frequency_options.push(default_frequency);
            frequency_options.sort_unstable();
        }
        let mut voltage_options = Vec::new();
        for n in (min..=max).step_by(25) {
            let _ = voltage_options.push(n);
        }
        Self {
            asic_model: model,
            device_model: device,
            swarm_color: "purple",
            asic_count: 1,
            default_frequency,
            default_voltage,
            frequency_options,
            voltage_options,
        }
    }
}

pub const COLUMNS: &[&str] = &["hashrate", "hashrate_1m", "power", "asicTemp"];
/// Query labels are whitelisted, bounded, and timestamp is always last. Unknown measurements are null.
pub fn columns(query: &str) -> Result<Vec<&str, 24>, &'static str> {
    let mut result = Vec::new();
    for field in query.split('&') {
        if let Some(value) = field.strip_prefix("columns=") {
            for label in value
                .split([','])
                .flat_map(|v| v.split("%2C"))
                .flat_map(|v| v.split("%2c"))
            {
                if label == "timestamp" || result.contains(&label) {
                    continue;
                }
                if label.is_empty()
                    || label.len() > 32
                    || !label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err("Colonne invalide.");
                }
                result.push(label).map_err(|_| "Trop de colonnes.")?;
            }
        }
    }
    if result.is_empty() {
        for label in COLUMNS {
            result.push(label).unwrap();
        }
    }
    result.push("timestamp").map_err(|_| "Trop de colonnes.")?;
    Ok(result)
}
pub struct Statistics<'a> {
    pub history: &'a History,
    pub labels: Vec<&'a str, 24>,
    pub now_ms: u64,
}
struct Rows<'a, 'b>(&'a Statistics<'b>);
impl Serialize for Rows<'_, '_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut rows = s.serialize_seq(Some(self.0.history.len()))?;
        for sample in self.0.history {
            let mut row = Vec::<Option<f64>, 24>::new();
            for label in &self.0.labels {
                row.push(match *label {
                    "timestamp" => Some(sample.timestamp as f64 * 1000.0),
                    "hashrate" | "hashrate_1m" => sample.hash_rate,
                    "power" => sample.power.map(f64::from),
                    "asicTemp" => sample.temp.map(f64::from),
                    _ => None,
                })
                .unwrap();
            }
            rows.serialize_element(&row)?;
        }
        rows.end()
    }
}
impl Serialize for Statistics<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(3))?;
        m.serialize_entry("currentTimestamp", &self.now_ms)?;
        m.serialize_entry("labels", &self.labels)?;
        m.serialize_entry("statistics", &Rows(self))?;
        m.end()
    }
}
