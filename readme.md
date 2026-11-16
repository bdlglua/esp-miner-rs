# ESP-Miner-rs

## Introduction
ESP-Miner-rs is bitcoin miner firmware designed to run on the ESP32-S3 of various Bitaxe:
- [BitaxeMax] with 1 Bitmain ASIC BM1397
- [BitaxeUltra] with 1 Bitmain ASIC BM1366
- [BitaxeGamma] 601 with 1 Bitmain ASIC BM1370 (TPS546 power control)

Gamma 601 setup and validation notes: [INTEGRATION-BM1370.md](INTEGRATION-BM1370.md).

## AxeOS Rust

A local Rust/WebAssembly interface is embedded in the ESP32 firmware. It provides live telemetry, pool/network/hardware settings stored in flash, pause/resume, event logs and restart. See [AXEOS-RUST.md](AXEOS-RUST.md) for the initial partition-table installation, features and current limits.

## Architecture
- `firmware/`: the `#[no_std]` firmware for the ESP32-S3 ([esp-hal] + [embassy]), built from its own directory
- `axeos-web/`: browser interface compiled from Rust to WebAssembly
- `axeos-core/`: shared settings, HTTP parsing and flash records (`no_std`)
- `gamma-power/`: the host-tested TPS546D24A PMBus driver for Gamma 601
- `vendor/bm13xx-rs/`: the supplied ASIC driver snapshot, used through local paths
- `miner-core/`: the hardware independent mining logic (block header, share check, job book, core voltage, fan curve), tested on the host
- ASIC driver: [bm13xx-rs]

[esp-hal]:https://github.com/esp-rs/esp-hal
[embassy]:https://github.com/embassy-rs/embassy
[bm13xx-rs]:https://github.com/GPTechinno/bm13xx-rs

## Setup

### Nix user
```bash
nix-shell
```

### Other
- install [rust](https://esp-rs.github.io/book/installation/installation.html#rust-installation)
- follow the [espup](https://esp-rs.github.io/book/installation/installation.html#espup) installation guide
    * skip §3.2 "RISC-V Targets Only" as out target is Xtensa ESP32-S3
    * in §3.3 "RISC-V and Xtensa Targets" you can install only esp32s3 target `espup install -t esp32s3`
    * skip §3.4 "std Development Requirements" as we are `#[no_std]`
- install [probe-rs](https://docs.esp-rs.org/book/tooling/debugging/probe-rs.html) is you are using `USB-JTAG-SERIAL` peripheral (recommanded) or [espflash](https://esp-rs.github.io/book/tooling/espflash.html)

## Build and/or Run

dev profile can potentially be one or more orders of magnitude slower than release, and may cause issues with timing-senstive peripherals and/or devices of `esp-hal`

### Using Just
- edit the just file to change your local WIFI SSID/PASSWORD
- `just build max` to build esp-miner-rs for [BitaxeMax]
- `just build ultra` to build esp-miner-rs for [BitaxeUltra]
- `just build gamma` to build esp-miner-rs for [BitaxeGamma] 601
- `just run max` to build/flash/run esp-miner-rs for [BitaxeMax] on the connected hardware
- `just test` to run the mining logic, Gamma power and BM1370 integration tests on the host

### Manually
- Configure the pool and your wallet/worker in the embedded AxeOS web interface after installing the supplied partition table (see `AXEOS-RUST.md`). No example wallet is used.
- `cd firmware`
- `WIFI_SSID="my_ssid" WIFI_PASSWORD="my_password" cargo build --release --features=bitaxe-max`
- `WIFI_SSID="my_ssid" WIFI_PASSWORD="my_password" cargo run --release --features=bitaxe-max`
- `WIFI_SSID="my_ssid" WIFI_PASSWORD="my_password" cargo build --release --features=bitaxe-gamma` for Gamma 601
- `cargo test` from the repository root runs all host tests

[BitaxeMax]:https://github.com/skot/bitaxe/tree/max-v2.3
[BitaxeUltra]:https://github.com/skot/bitaxe/tree/ultra-204
[BitaxeGamma]:https://github.com/bitaxeorg/bitaxeGamma
