//! Volatile PMBus configuration for the single-phase, 5 V Bitaxe Gamma 601.
//!
//! Based on ESP-Miner's TPS546_CONFIG_DEFAULT. Never writes regulator NVM.
#![no_std]

use embedded_hal_async::i2c::I2c;

pub const ADDRESS: u8 = 0x24;

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Bus(E),
    DeviceId,
    VoltageMode,
    VoltageRange,
    Readback,
}

#[derive(Debug)]
pub struct Telemetry {
    pub vin: f32,
    pub vout: f32,
    pub iout: f32,
    pub temperature: f32,
    pub status: u16,
}

pub struct Tps546<I> {
    bus: I,
    exponent: i8,
}

impl<I: I2c> Tps546<I> {
    pub fn new(bus: I) -> Self {
        Self { bus, exponent: 0 }
    }

    async fn byte(&mut self, command: u8, value: u8) -> Result<(), Error<I::Error>> {
        self.bus
            .write(ADDRESS, &[command, value])
            .await
            .map_err(Error::Bus)
    }

    async fn word(&mut self, command: u8, value: u16) -> Result<(), Error<I::Error>> {
        let [lo, hi] = value.to_le_bytes();
        self.bus
            .write(ADDRESS, &[command, lo, hi])
            .await
            .map_err(Error::Bus)
    }

    async fn read_byte(&mut self, command: u8) -> Result<u8, Error<I::Error>> {
        let mut data = [0];
        self.bus
            .write_read(ADDRESS, &[command], &mut data)
            .await
            .map_err(Error::Bus)?;
        Ok(data[0])
    }

    async fn read_word(&mut self, command: u8) -> Result<u16, Error<I::Error>> {
        let mut data = [0; 2];
        self.bus
            .write_read(ADDRESS, &[command], &mut data)
            .await
            .map_err(Error::Bus)?;
        Ok(u16::from_le_bytes(data))
    }

    async fn voltage(&mut self, command: u8, volts: f32) -> Result<(), Error<I::Error>> {
        let raw = encode_voltage(volts, self.exponent).ok_or(Error::VoltageRange)?;
        self.word(command, raw).await
    }

    /// Leaves the output OFF. Caller must allow at least 15 ms after power-up before calling.
    pub async fn init(&mut self, volts: f32) -> Result<(), Error<I::Error>> {
        if !volts.is_finite() || !(1.0..=1.25).contains(&volts) {
            return Err(Error::VoltageRange);
        }
        self.disable().await?;
        let mut id = [0; 7]; // SMBus block: count followed by the six ID bytes.
        self.bus
            .write_read(ADDRESS, &[0xad], &mut id)
            .await
            .map_err(Error::Bus)?;
        if id != [6, 0x54, 0x49, 0x54, 0x6d, 0x24, 0x41]
            && id != [6, 0x54, 0x49, 0x54, 0x6d, 0x24, 0x62]
        {
            return Err(Error::DeviceId);
        }
        let mode = self.read_byte(0x20).await?;
        if mode & 0xe0 != 0 {
            return Err(Error::VoltageMode);
        }
        self.exponent = ((mode << 3) as i8) >> 3;
        // Command-controlled output; Gamma has no ASIC enable GPIO.
        self.byte(0x02, 0x1b).await?;
        self.word(0xec, 0).await?; // single-phase stack
        self.byte(0xe4, 0x10).await?; // SYNC_CONFIG
        self.byte(0x04, 0).await?; // PHASE
        for (command, value) in [
            (0x33, 650.0), // switching frequency, kHz
            (0x35, 4.8),
            (0x36, 4.5),  // VIN on/off
            (0x55, 6.5),  // input overvoltage
            (0x29, 0.25), // VOUT_SCALE_LOOP
            (0x4a, 25.0),
            (0x46, 30.0), // output current warn/fault, A
            (0x51, 105.0),
            (0x4f, 145.0), // regulator temperature warn/fault, C
            (0x60, 0.0),
            (0x61, 3.0),
            (0x62, 0.0), // TON timings, ms
            (0x64, 0.0),
            (0x65, 0.0), // TOFF timings, ms
        ] {
            self.word(command, encode_linear11(value)).await?;
        }
        self.byte(0x56, 0xb7).await?; // VIN_OV fault response
        self.byte(0x47, 0xc0).await?; // overcurrent: latch off
        self.byte(0x50, 0xff).await?; // regulator overtemperature response
        self.byte(0x63, 0x3b).await?;
        self.voltage(0x24, 1.25).await?; // tighter BM1370 VOUT_MAX
        self.voltage(0x2b, 1.0).await?;
        // TPS546 voltage limits/margins are ratios to VOUT_COMMAND.
        for (command, ratio) in [
            (0x40, 1.25),
            (0x42, 1.16),
            (0x25, 1.1),
            (0x26, 0.90),
            (0x43, 0.90),
            (0x44, 0.75),
        ] {
            self.voltage(command, ratio).await?;
        }
        self.voltage(0x21, volts).await?;
        self.word(0xee, 0xffff).await?; // use pin-detected boot values; no STORE_USER_ALL
        self.bus.write(ADDRESS, &[0x03]).await.map_err(Error::Bus)?; // CLEAR_FAULTS
        let expected = encode_voltage(volts, self.exponent).ok_or(Error::VoltageRange)?;
        if self.read_word(0x21).await? != expected || self.read_byte(0x01).await? != 0 {
            return Err(Error::Readback);
        }
        Ok(())
    }

    pub async fn enable(&mut self) -> Result<(), Error<I::Error>> {
        self.byte(0x01, 0x80).await?;
        if self.read_byte(0x01).await? != 0x80 {
            return Err(Error::Readback);
        }
        Ok(())
    }

    pub async fn disable(&mut self) -> Result<(), Error<I::Error>> {
        self.byte(0x01, 0).await
    }

    pub async fn telemetry(&mut self) -> Result<Telemetry, Error<I::Error>> {
        Ok(Telemetry {
            vin: decode_linear11(self.read_word(0x88).await?),
            vout: self.read_word(0x8b).await? as f32 * pow2(self.exponent),
            iout: decode_linear11(self.read_word(0x8c).await?),
            temperature: decode_linear11(self.read_word(0x8d).await?),
            status: self.read_word(0x79).await?,
        })
    }
}

fn pow2(exponent: i8) -> f32 {
    if exponent >= 0 {
        (1u32 << exponent as u32) as f32
    } else {
        1.0 / (1u32 << (-exponent) as u32) as f32
    }
}

fn encode_voltage(volts: f32, exponent: i8) -> Option<u16> {
    let value = volts / pow2(exponent);
    if !value.is_finite() || !(0.0..=65535.0).contains(&value) {
        None
    } else {
        Some((value + 0.5) as u16)
    }
}

fn decode_linear11(raw: u16) -> f32 {
    let exponent = (raw as i16 >> 11) as i8;
    let mantissa = (raw << 5) as i16 >> 5;
    mantissa as f32 * pow2(exponent)
}

// The profile values above are positive and fit in LINEAR11's [-16, 15] exponent range.
fn encode_linear11(value: f32) -> u16 {
    let mut exponent = -16i8;
    let mut mantissa = value / pow2(exponent);
    while mantissa > 1023.0 && exponent < 15 {
        exponent += 1;
        mantissa = value / pow2(exponent);
    }
    (((exponent as u16) & 0x1f) << 11) | ((mantissa + 0.5) as u16 & 0x7ff)
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;
