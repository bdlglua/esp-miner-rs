# ESP-Miner-rs

## Introduction
ESP-Miner-rs is bitcoin miner firmware designed to run on the ESP32-S3 of various Bitaxe:

- [BitaxeMax] with 1 Bitmain ASIC BM1397
- [BitaxeUltra] with 1 Bitmain ASIC BM1366
- [BitaxeGamma] with 1 Bitmain ASIC BM1370

and more to come.

This fork adds Gamma 601 power management and the official AxeOS interface, served by the Rust firmware. Compilation and host tests have been completed; validation on physical hardware is still required. See [integration status](docs/status.md) for supported features and current limitations.

## Architecture

- `firmware/`: the `#[no_std]` firmware for the ESP32-S3 ([esp-hal] + [embassy]), built from its own directory
- `crates/miner-core/`: the hardware independent mining logic (block header, share check, job book, core voltage, fan curve), tested on the host
- `web/axeos/`: the official Angular AxeOS interface, connected to the Rust API
- ASIC driver: [bm13xx-rs], included in `vendor/bm13xx-rs/` with BM1370 support

[esp-hal]:https://github.com/esp-rs/esp-hal
[embassy]:https://github.com/embassy-rs/embassy
[bm13xx-rs]:https://github.com/GPTechinno/bm13xx-rs

## Setup

- install [rust](https://esp-rs.github.io/book/installation/installation.html#rust-installation)
- follow the [espup](https://esp-rs.github.io/book/installation/installation.html#espup) installation guide
    * skip §3.2 "RISC-V Targets Only" as our target is Xtensa ESP32-S3
    * in §3.3 "RISC-V and Xtensa Targets" you can install only esp32s3 target `espup install -t esp32s3`
    * skip §3.4 "std Development Requirements" as we are `#[no_std]`
- install [probe-rs](https://docs.esp-rs.org/book/tooling/debugging/probe-rs.html) if you are using `USB-JTAG-SERIAL` peripheral (recommended) or [espflash](https://esp-rs.github.io/book/tooling/espflash.html)

## Build and/or Run

dev profile can potentially be one or more orders of magnitude slower than release, and may cause issues with timing-sensitive peripherals and/or devices of `esp-hal`

### Using Just

- set your local Wi-Fi credentials: `export WIFI_SSID="my_ssid" WIFI_PASSWORD="my_password"`
- `just build max` to build esp-miner-rs for [BitaxeMax]
- `just build ultra` to build esp-miner-rs for [BitaxeUltra]
- `just build gamma` to build esp-miner-rs for [BitaxeGamma] 601
- `just test` to run the host workspace tests

### Manually

- `cd firmware`
- `WIFI_SSID="my_ssid" WIFI_PASSWORD="my_password" cargo build --release --features=bitaxe-max`
- `WIFI_SSID="my_ssid" WIFI_PASSWORD="my_password" cargo build --locked --release --features=bitaxe-gamma`
- `WIFI_SSID="my_ssid" WIFI_PASSWORD="my_password" cargo run --release --features=bitaxe-max`
- `cargo test` from the repository root runs the host workspace tests

For the initial USB installation, run the following from `firmware/` with the supplied partition table:

```bash
espflash flash --partition-table partitions.csv \
  target/xtensa-esp32s3-none-elf/release/esp-miner-rs
```

Then open the device’s DHCP IPv4 address in your browser. Gamma 601 defaults to 525 MHz / 1,150 mV. See the [build guide](docs/build.md) for installation details.

## AxeOS

The official production interface is already embedded in the firmware. To preview AxeOS without hardware, use Node.js 22:

```bash
cd web/axeos
npm ci
npm run generate:api
npm start
```

Open http://localhost:4200. This preview uses the official mock data.

From the repository root, `python3 scripts/build-axeos.py` rebuilds the production interface and its embedded assets; Python 3.11+ is required. Rebuild the firmware afterwards. The `--demo` option builds a mock preview without replacing the embedded production assets.

AxeOS retains its [upstream license](web/LICENSE-ESP-Miner). Source provenance is recorded in [docs/upstream.json](docs/upstream.json).

[BitaxeMax]:https://github.com/skot/bitaxe/tree/max-v2.3
[BitaxeUltra]:https://github.com/skot/bitaxe/tree/ultra-204
[BitaxeGamma]:docs/bm1370.md
