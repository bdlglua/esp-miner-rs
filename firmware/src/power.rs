//! Core power, ASIC temperature and fan.

use bounded_integer::BoundedU8;
#[cfg(not(feature = "bitaxe-gamma"))]
use defmt::warn;
use defmt::{error, info};
#[cfg(not(feature = "bitaxe-gamma"))]
use ds4432::{AsyncDS4432, Status};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    signal::Signal,
};
use embassy_time::{Duration, Timer};
use emc2101::AsyncEMC2101;
#[cfg(not(feature = "bitaxe-gamma"))]
use esp_hal::gpio::Output;
use esp_hal::{i2c::master::I2c, Async};
use fugit::HertzU32;
#[cfg(not(feature = "bitaxe-gamma"))]
use ina260::AsyncINA260;
use miner_core::thermal;
#[cfg(not(feature = "bitaxe-gamma"))]
use miner_core::vcore;

use crate::board;

/// A device on the shared I²C bus.
pub type BusDevice = I2cDevice<'static, NoopRawMutex, I2c<'static, Async>>;

/// Signaled once the core voltage is up, so the chips can be brought up.
pub static POWER_READY: Signal<CriticalSectionRawMutex, ()> = Signal::new();

const MONITOR_PERIOD: Duration = Duration::from_secs(5);
/// Time for the regulator to settle after it is enabled.
const VCORE_SETTLE: Duration = Duration::from_millis(100);
const FAN_PWM_FREQ: HertzU32 = HertzU32::Hz(5806);

/// Sets the core voltage, enables the regulator, then watches the ASIC temperature: drives the fan
/// and cuts the core power above [`thermal::OVERHEAT_C`].
///
/// `asic_enable` is the regulator enable (PWR_EN), active low.
#[cfg(not(feature = "bitaxe-gamma"))]
#[embassy_executor::task]
pub async fn power_task(
    mut dac: AsyncDS4432<BusDevice>,
    mut monitor: AsyncINA260<BusDevice>,
    mut sensor: AsyncEMC2101<BusDevice>,
    mut asic_enable: Output<'static>,
) {
    // full speed until the first temperature reading
    if let Err(e) = sensor.set_fan_pwm(FAN_PWM_FREQ, false).await {
        error!("fan PWM setup failed: {}", e);
    }
    set_fan(&mut sensor, 100).await;

    if crate::state::with(|s| s.telemetry.fault) {
        return;
    }

    let target_voltage = crate::state::with(|s| s.active.core_voltage as f32 / 1000.0);
    // Set the DAC before enabling the regulator, so it starts at the target voltage instead of
    // its nominal ~1.5 V.
    let current_ua =
        vcore::feedback_current_ua(target_voltage, board::VCORE_RA_OHM, board::VCORE_RB_OHM);
    let status = if current_ua >= 0.0 {
        Status::SourceMicroAmp(current_ua)
    } else {
        Status::SinkMicroAmp(-current_ua)
    };
    if let Err(e) = dac.set_status(ds4432::Output::Zero, status).await {
        crate::state::fault("Réglage de la tension DS4432 impossible");
        error!(
            "setting the core voltage failed, core power stays off: {}",
            e
        );
        return;
    }
    asic_enable.set_low();
    info!("core voltage set to {} V", target_voltage);
    Timer::after(VCORE_SETTLE).await;
    crate::state::with(|s| s.telemetry.power_ready = true);
    POWER_READY.signal(());

    loop {
        Timer::after(MONITOR_PERIOD).await;

        let temp = asic_temp(&mut sensor).await;
        match (
            monitor.power().await,
            monitor.current().await,
            monitor.voltage().await,
        ) {
            (Ok(mw), Ok(ua), Ok(uv)) => {
                info!("ASIC {} C, input {} mW", temp, mw);
                crate::state::with(|s| {
                    s.telemetry.power = Some(mw as f32 / 1000.0);
                    s.telemetry.voltage = Some(uv as f32 / 1000.0);
                    s.telemetry.current = Some(ua as f32 / 1000.0);
                });
            }
            _ => warn!("ASIC {} °C, reading the INA260 failed", temp),
        }

        crate::state::with(|s| s.telemetry.temp = temp.is_finite().then_some(temp));
        if !temp.is_finite() || temp > thermal::OVERHEAT_C {
            asic_enable.set_high();
            crate::state::fault("Surchauffe ou température indisponible : alimentation coupée");
            set_fan(&mut sensor, 100).await;
            error!(
                "ASIC overheat ({} °C > {} °C): core power cut until reboot",
                temp,
                thermal::OVERHEAT_C
            );
            return;
        }
        set_fan(&mut sensor, fan_target(temp)).await;
    }
}

/// ASIC temperature (°C), NaN when it can't be read.
async fn asic_temp(sensor: &mut AsyncEMC2101<BusDevice>) -> f32 {
    #[cfg(feature = "bitaxe-gamma")]
    match sensor.status().await {
        Ok(status) if !status.ext_diode_fault => {}
        _ => {
            error!("Gamma external temperature diode unavailable");
            return f32::NAN;
        }
    }
    let temp = if board::ASIC_TEMP_INTERNAL {
        sensor.temp_internal().await.map(f32::from)
    } else {
        sensor.temp_external_precise().await
    };
    match temp {
        Ok(t) => t + board::ASIC_TEMP_OFFSET_C,
        Err(e) => {
            error!("reading the ASIC temperature failed: {}", e);
            f32::NAN
        }
    }
}

async fn set_fan(sensor: &mut AsyncEMC2101<BusDevice>, percent: u8) -> bool {
    let step = BoundedU8::new(thermal::emc2101_fan_step(percent)).unwrap_or(BoundedU8::MAX);
    if let Err(e) = sensor.set_fan_power(step).await {
        error!("setting the fan to {}% failed: {}", percent, e);
        return false;
    }
    let rpm = sensor.fan_rpm().await.ok();
    crate::state::with(|s| {
        s.telemetry.fan_percent = percent;
        s.telemetry.fan_rpm = rpm;
    });
    true
}
fn fan_target(temp: f32) -> u8 {
    crate::state::with(|s| {
        if s.config.auto_fan || temp >= 70.0 {
            thermal::fan_percent(temp)
        } else {
            s.config.fan_percent
        }
    })
}

/// Gamma 601: all core power and telemetry go through the TPS546, not DS4432/INA260.
#[cfg(feature = "bitaxe-gamma")]
#[embassy_executor::task]
pub async fn gamma_power_task(
    mut regulator: gamma_power::Tps546<BusDevice>,
    mut sensor: AsyncEMC2101<BusDevice>,
) {
    let target_voltage = crate::state::with(|s| s.active.core_voltage as f32 / 1000.0);
    Timer::after(Duration::from_millis(15)).await;
    if regulator.disable().await.is_err() {
        crate::state::fault("Arrêt du TPS546 impossible");
        error!("TPS546 output disable failed; mining will not start");
        return;
    }
    if crate::state::with(|s| s.telemetry.fault) {
        return;
    }
    if sensor.set_fan_pwm(FAN_PWM_FREQ, false).await.is_err()
        || sensor.set_fan_power(BoundedU8::MAX).await.is_err()
    {
        crate::state::fault("Initialisation ventilateur Gamma impossible");
        error!("Gamma fan setup failed; core power stays off");
        return;
    }
    // Allow a fresh diode conversion after the calibration writes in main.
    Timer::after(VCORE_SETTLE).await;
    let temp = asic_temp(&mut sensor).await;
    if !(-40.0..=thermal::OVERHEAT_C).contains(&temp) {
        crate::state::fault("Température Gamma invalide ou excessive");
        error!("Gamma temperature invalid or too high; core power stays off");
        return;
    }
    if regulator.init(target_voltage).await.is_err() || regulator.enable().await.is_err() {
        let _ = regulator.disable().await;
        crate::state::fault("Initialisation ou relecture TPS546 impossible");
        error!("TPS546 setup/readback failed; mining will not start");
        return;
    }
    Timer::after(VCORE_SETTLE).await;
    // Verify actual output before releasing the ASIC task, then on every monitor cycle.
    let mut started = false;
    loop {
        let temp = asic_temp(&mut sensor).await;
        let healthy = match regulator.telemetry().await {
            Ok(t) => {
                info!(
                    "ASIC {} C, TPS546 {} C, input {} V, core {} V / {} A, status {:#06x}",
                    temp, t.temperature, t.vin, t.vout, t.iout, t.status
                );
                crate::state::with(|s| {
                    s.telemetry.temp = temp.is_finite().then_some(temp);
                    s.telemetry.vr_temp = Some(t.temperature);
                    s.telemetry.power = Some(t.vout * t.iout);
                    s.telemetry.voltage = Some(t.vin * 1000.0);
                    s.telemetry.current = Some(t.iout * 1000.0);
                    s.telemetry.core_voltage_actual = Some(t.vout * 1000.0);
                });
                (4.5..=6.5).contains(&t.vin)
                    && (target_voltage * 0.9..=target_voltage * 1.1).contains(&t.vout)
                    && t.temperature < 105.0
                    && t.status == 0
            }
            Err(_) => false,
        };
        if !healthy || !(-40.0..=thermal::OVERHEAT_C).contains(&temp) {
            if regulator.disable().await.is_err() {
                error!("TPS546 shutdown command failed");
            }
            set_fan(&mut sensor, 100).await;
            crate::state::fault("Défaut alimentation ou température Gamma");
            error!("Gamma power/temperature fault: mining stopped until reboot");
            return;
        }
        if !set_fan(&mut sensor, fan_target(temp)).await {
            let _ = regulator.disable().await;
            crate::state::fault("Commande du ventilateur impossible");
            error!("Gamma fan command failed: core power cut until reboot");
            return;
        }
        if !started {
            started = true;
            info!("Gamma core voltage verified at {} V", target_voltage);
            crate::state::with(|s| s.telemetry.power_ready = true);
            POWER_READY.signal(());
        }
        Timer::after(MONITOR_PERIOD).await;
    }
}
