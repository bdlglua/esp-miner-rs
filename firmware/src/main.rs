#![no_std]
#![no_main]

mod asic;
mod board;
mod io06;
mod pool;
mod power;
mod settings;
mod state;
mod web;

use defmt::{debug, error, info};
use defmt_rtt as _;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_net::{Runner, StackResources};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    i2c::master::I2c,
    rng::Rng,
    time::Rate,
    timer::timg::TimerGroup,
    uart::{self, Uart},
};
use esp_radio::wifi::{
    sta::StationConfig, AuthenticationMethodConfig, Config, ControllerConfig, Interface,
    WifiController,
};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    // Keep the ASIC in reset throughout power and sensor initialization.
    let reset = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());

    esp_alloc::heap_allocator!(size: 72 * 1024);
    let defaults = axeos_core::Config::defaults(
        WIFI_SSID,
        WIFI_PASSWORD,
        board::HASH_FREQ_MHZ as u16,
        (board::VCORE * 1000.0) as u16,
    );
    let (store, config) =
        settings::SettingsStore::load(esp_storage::FlashStorage::new(peripherals.FLASH), defaults);
    state::initialize(config.clone(), store.available());
    settings::initialize(store);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    // Shared I2C0 (SDA GPIO47, SCL GPIO48): EMC2101 plus DS4432/INA260 on Max/Ultra,
    // TPS546 on Gamma 601. Gamma controls power through PMBus, without GPIO10.
    static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2c<'_, esp_hal::Async>>> = StaticCell::new();
    let i2c = I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO47)
    .with_scl(peripherals.GPIO48)
    .into_async();
    let i2c_bus = I2C_BUS.init(Mutex::new(i2c));
    #[cfg(not(feature = "bitaxe-gamma"))]
    let ds4432 =
        ds4432::AsyncDS4432::with_rfs(I2cDevice::new(i2c_bus), Some(board::DS4432_RFS0_OHM), None)
            .unwrap();
    #[cfg(not(feature = "bitaxe-gamma"))]
    let ina260 = match ina260::AsyncINA260::new(I2cDevice::new(i2c_bus)).await {
        Ok(monitor) => Some(monitor),
        Err(_) => {
            state::fault("Moniteur INA260 indisponible");
            None
        }
    };
    #[cfg(feature = "bitaxe-gamma")]
    let mut regulator = gamma_power::Tps546::new(I2cDevice::new(i2c_bus));
    #[cfg(feature = "bitaxe-gamma")]
    {
        Timer::after(Duration::from_millis(15)).await;
        if regulator.disable().await.is_err() {
            state::fault("Arrêt initial du TPS546 impossible");
        }
    }
    // Gamma 601: BM1370 external diode calibration (EMC2101 registers 0x17/0x18).
    #[cfg(feature = "bitaxe-gamma")]
    {
        let mut sensor_bus = I2cDevice::new(i2c_bus);
        if embedded_hal_async::i2c::I2c::write(&mut sensor_bus, 0x4c, &[0x17, 0x24])
            .await
            .is_err()
            || embedded_hal_async::i2c::I2c::write(&mut sensor_bus, 0x4c, &[0x18, 0x00])
                .await
                .is_err()
        {
            state::fault("Calibration de la diode BM1370 impossible");
        }
    }
    let emc2101 = match emc2101::AsyncEMC2101::new(I2cDevice::new(i2c_bus)).await {
        Ok(sensor) => Some(sensor),
        Err(_) => {
            state::fault("Capteur et contrôleur de ventilateur EMC2101 indisponible");
            None
        }
    };
    #[cfg(not(feature = "bitaxe-gamma"))]
    let asic_enable = Output::new(peripherals.GPIO10, Level::High, OutputConfig::default());
    #[cfg(not(feature = "bitaxe-gamma"))]
    if let (Some(monitor), Some(sensor)) = (ina260, emc2101) {
        spawner.spawn(power::power_task(ds4432, monitor, sensor, asic_enable).unwrap());
    }

    #[cfg(feature = "bitaxe-gamma")]
    if let Some(sensor) = emc2101 {
        spawner.spawn(power::gamma_power_task(regulator, sensor).unwrap());
    }

    // ASIC chain: UART1 (TX GPIO17, RX GPIO18), NRST on GPIO1 (active low).
    let uart_config = uart::Config::default().with_baudrate(115_200);
    let uart = Uart::new(peripherals.UART1, uart_config)
        .unwrap()
        .with_tx(peripherals.GPIO17)
        .with_rx(peripherals.GPIO18)
        .into_async();
    spawner.spawn(asic::asic_task(asic::AsicUart::new(uart, uart_config), reset).unwrap());

    // WiFi and network stack
    let station_config = Config::Station(
        StationConfig::default()
            .with_ssid(config.ssid.as_str().try_into().unwrap())
            .with_authentication(if config.wifi_password.is_empty() {
                AuthenticationMethodConfig::Open
            } else {
                AuthenticationMethodConfig::Wpa2Personal(
                    config.wifi_password.as_str().try_into().unwrap(),
                )
            }),
    );
    let wifi_interface = Interface::station();
    let controller = WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config(station_config),
    )
    .unwrap();

    let mut dhcp = embassy_net::DhcpConfig::default();
    dhcp.hostname = config.hostname.as_str().try_into().ok();
    let config = embassy_net::Config::dhcpv4(dhcp);

    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<8>, StackResources::<8>::new()),
        seed,
    );

    spawner.spawn(connection_task(controller).unwrap());
    spawner.spawn(net_task(runner).unwrap());

    spawner.spawn(web::web_task(stack).unwrap());
    spawner.spawn(web::web_task(stack).unwrap());
    spawner.spawn(web::web_task(stack).unwrap());
    spawner.spawn(web::web_task(stack).unwrap());
    spawner.spawn(web::stats_task(stack).unwrap());

    // Pool session
    spawner.spawn(
        pool::pool_task(
            stack,
            mk_static!([u8; 1536], [0; 1536]),
            mk_static!([u8; 1536], [0; 1536]),
        )
        .unwrap(),
    );

    stack.wait_config_up().await;
    if let Some(config) = stack.config_v4() {
        info!("Got IP: {}", config.address);
    }

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}

#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) {
    debug!("start connection task");
    loop {
        debug!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => {
                debug!("Wifi connected!");
                // wait until we're no longer connected
                controller.wait_for_disconnect_async().await.ok();
                debug!("Wifi disconnected");
            }
            Err(e) => {
                error!("Failed to connect to wifi: {:?}", e);
            }
        }
        Timer::after(Duration::from_millis(5000)).await
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await
}
