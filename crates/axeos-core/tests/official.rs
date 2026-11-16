use axeos_core::{
    official::{self, SettingsError, SystemInfo},
    Config, History, Info, Sample, Telemetry,
};
use serde_json::{json, Value};
fn config() -> Config {
    Config::defaults("TestSSID", "password-secret", 525, 1150)
}
fn info() -> Info<'static> {
    let mut c = config();
    c.pool_password = "pool-secret".try_into().unwrap();
    Info {
        device_model: "Gamma",
        asic_model: "BM1370",
        board_version: "601",
        version: "0.1.0",
        axe_os_version: "rust-0.1.0",
        uptime_seconds: 42,
        free_heap: 4096,
        ipv4: "192.168.1.5".try_into().unwrap(),
        wifi_connected: true,
        storage_available: true,
        restart_required: false,
        voltage_min: 1000,
        voltage_max: 1250,
        settings: c.public(),
        telemetry: Telemetry {
            temp: Some(53.5),
            power: Some(12.5),
            voltage: Some(5000.0),
            current: Some(2500.0),
            core_voltage_actual: Some(1149.0),
            hash_rate: Some(1200.5),
            ..Telemetry::default()
        },
    }
}
fn apply(payload: Value, c: &Config) -> Result<Config, SettingsError> {
    official::parse_settings(&serde_json::to_vec(&payload).unwrap())?.apply(c, 1000, 1250, false)
}
#[test]
fn flat_info_matches_upstream_required_names_and_types_without_leaking_secrets() {
    // Use the actual no_std firmware serializer, not just serde_json's host serializer.
    let mut bytes = [0; 12288];
    let i = info();
    let n = serde_json_core::to_slice(&SystemInfo(&i), &mut bytes).unwrap();
    let text = core::str::from_utf8(&bytes[..n]).unwrap();
    let v: Value = serde_json::from_str(text).unwrap();
    assert!(!text.contains("password-secret"));
    assert!(!text.contains("pool-secret"));
    assert_eq!(v["ASICModel"], "BM1370");
    assert_eq!(v["voltage"], 5000.0);
    assert_eq!(v["coreVoltageActual"], 1149.0);
    assert_eq!(v["hashRate"], 1200.5);
    assert_eq!(v["pools"][0]["stratumPassword"], "*****");
    assert!(v.get("settings").is_none());
    let schema = include_str!("../../../web/openapi.yaml")
        .split("    SystemInfo:\n")
        .nth(1)
        .unwrap()
        .split("    SystemASIC:")
        .next()
        .unwrap();
    let required = schema
        .split("      required:\n")
        .nth(1)
        .unwrap()
        .split("      properties:")
        .next()
        .unwrap();
    let properties = schema.split_once("      properties:\n").unwrap().1;
    for line in required.lines() {
        let Some(key) = line.trim().strip_prefix("- ") else {
            continue;
        };
        let value = v
            .get(key)
            .unwrap_or_else(|| panic!("Missing required field {key}"));
        let marker = format!("        {key}:\n");
        let entry = properties.split(&marker).nth(1).unwrap();
        let kind = entry
            .lines()
            .next()
            .unwrap()
            .trim()
            .strip_prefix("type: ")
            .unwrap();
        assert!(
            match kind {
                "number" | "integer" => value.is_number(),
                "string" => value.is_string(),
                "boolean" => value.is_boolean(),
                "array" => value.is_array(),
                "object" => value.is_object(),
                _ => panic!("Unknown type {kind}"),
            },
            "Wrong upstream type for {key}: {value}"
        );
    }
}
#[test]
fn complete_hardware_form_and_network_form_are_accepted() {
    let c = config();
    let hardware = json!({"display":"NONE","rotation":0,"invertscreen":false,"displayTimeout":-1,"coreVoltage":1150,"frequency":500,"autofanspeed":false,"minFanSpeed":30,"manualFanSpeed":80,"temptarget":60,"overheat_mode":0,"statsFrequency":5});
    let next = apply(hardware, &c).unwrap();
    assert_eq!(next.frequency, 500);
    assert!(!next.auto_fan);
    assert_eq!(next.fan_percent, 80);
    let next = apply(
        json!({"hostname":"my-gamma","ssid":"Réseau","wifiPass":"a\\b\"cd12345","useNTP":false}),
        &next,
    )
    .unwrap();
    assert_eq!(next.wifi_password, "a\\b\"cd12345");
    assert_eq!(next.ssid, "Réseau");
    let next = apply(json!({"autofanspeed":1}), &next).unwrap();
    assert!(next.auto_fan);
}
#[test]
fn official_pool_form_roundtrip_keeps_masked_password_and_changes_worker() {
    let c = config();
    let mut payload = serde_json::to_value(SystemInfo(&info())).unwrap();
    let pool = &mut payload["pools"][0];
    pool["stratumUser"] = json!("my-wallet.gamma");
    pool["stratumURL"] = json!("pool.example.org");
    let next = apply(
        json!({"pools":payload["pools"],"primaryPoolIndex":0,"secondaryPoolIndex":0}),
        &c,
    )
    .unwrap();
    assert_eq!(next.pool_host, "pool.example.org");
    assert_eq!(next.pool_user, "my-wallet.gamma");
    assert_eq!(next.pool_password, c.pool_password);
    assert_eq!(
        apply(json!({"wifiPass":"*****"}), &c)
            .unwrap()
            .wifi_password,
        c.wifi_password
    );
    assert!(apply(json!({"wifiPass":""}), &c)
        .unwrap()
        .wifi_password
        .is_empty());
}
#[test]
fn unsupported_changes_and_invalid_limits_are_atomic() {
    let c = config();
    for payload in [
        json!({"frequency":500,"useNTP":true}),
        json!({"frequency":500,"overclockEnabled":1}),
        json!({"statsFrequency":30}),
        json!({"temptarget":70}),
        json!({"stratumProtocol":"SV2"}),
        json!({"secondaryPoolIndex":1}),
    ] {
        assert!(matches!(
            apply(payload, &c),
            Err(SettingsError::Unsupported(_))
        ));
    }
    for payload in [
        json!({"coreVoltage":1251}),
        json!({"frequency":526}),
        json!({"manualFanSpeed":0}),
        json!({"ssid":"a".repeat(33)}),
        json!({"autofanspeed":2}),
        json!({"unknown":1}),
    ] {
        assert!(apply(payload, &c).is_err());
    }
    assert_eq!(c.frequency, 525);
    assert_eq!(c.core_voltage, 1150);
    assert!(official::parse_settings(b"{} garbage").is_err());
    assert!(official::parse_settings(&vec![b' '; 2049]).is_err());
    assert!(official::parse_settings(br#"{"overheat_mode":0}"#)
        .unwrap()
        .apply(&c, 1000, 1250, true)
        .is_err());
    assert!(apply(json!({"pools":[{"id":0,"stratumProtocol":"SV2"}]}), &c).is_err());
}
#[test]
fn statistics_column_order_units_and_gaps_match_official_charts() {
    let mut history = History::new();
    history
        .push(Sample {
            timestamp: 3,
            hash_rate: Some(1200.0),
            temp: None,
            power: Some(10.0),
        })
        .unwrap_or_else(|_| panic!());
    let labels = official::columns("columns=power%2Chashrate,asicTemp,vrTemp&ignored=1").unwrap();
    let v = serde_json::to_value(official::Statistics {
        history: &history,
        labels,
        now_ms: 5000,
    })
    .unwrap();
    assert_eq!(
        v["labels"],
        json!(["power", "hashrate", "asicTemp", "vrTemp", "timestamp"])
    );
    assert_eq!(v["statistics"], json!([[10.0, 1200.0, null, null, 3000.0]]));
    assert_eq!(v["currentTimestamp"], 5000);
    assert!(official::columns("columns=bad%00label").is_err());
    assert!(official::columns(&format!(
        "columns={}",
        (0..25)
            .map(|n| format!("x{n}"))
            .collect::<Vec<_>>()
            .join(",")
    ))
    .is_err());
}
#[test]
fn asic_menu_does_not_offer_unsafe_settings() {
    let a = official::AsicSettings::new("BM1370", "Gamma", 525, 1150, 1000, 1250);
    assert!(a.frequency_options.contains(&525));
    assert!(a.frequency_options.iter().all(|v| *v <= 525));
    assert!(a.voltage_options.iter().all(|v| (1000..=1250).contains(v)));
}
#[test]
fn websocket_upgrade_is_checked_and_empty_official_posts_work() {
    use axeos_core::http::{self, Error};
    let base="GET /api/ws/live HTTP/1.1\r\nHost: bitaxe\r\nUpgrade: websocket\r\nConnection: keep-alive, Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n";
    assert!(
        http::parse(format!("{base}Origin: http://bitaxe\r\n\r\n").as_bytes())
            .unwrap()
            .unwrap()
            .websocket_key
            .is_some()
    );
    assert!(matches!(
        http::parse(format!("{base}Origin: https://evil.example\r\n\r\n").as_bytes()),
        Err(Error::Forbidden)
    ));
    assert!(http::parse(
        b"POST /api/system/restart HTTP/1.1\r\nHost: bitaxe\r\nContent-Length: 0\r\n\r\n"
    )
    .unwrap()
    .is_some());
    assert!(
        http::parse(format!("{}\r\n", base.replace("Version: 13", "Version: 12")).as_bytes())
            .is_err()
    );
    assert_eq!(
        http::parse(
            b"GET /api/system/statistics?columns=power,hashrate HTTP/1.1\r\nHost: bitaxe\r\n\r\n"
        )
        .unwrap()
        .unwrap()
        .query,
        "columns=power,hashrate"
    );
}
