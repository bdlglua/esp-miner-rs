use super::*;
use embedded_hal_mock::eh1::i2c::{Mock, Transaction};
use futures::executor::block_on;

#[test]
fn pmbus_formats_cover_gamma_values_and_signed_telemetry() {
    assert_eq!(encode_voltage(1.15, -9), Some(589));
    assert_eq!(encode_voltage(1.15, -12), Some(4710));
    assert_eq!(encode_voltage(f32::NAN, -9), None);
    assert_eq!(encode_voltage(1000.0, -16), None);
    for value in [
        0.0, 0.25, 3.0, 4.5, 4.8, 6.5, 25.0, 30.0, 105.0, 145.0, 650.0,
    ] {
        assert!((decode_linear11(encode_linear11(value)) - value).abs() < 0.01);
    }
    assert_eq!(decode_linear11(0x07ff), -1.0);
    assert_eq!(decode_linear11(0xf7fc), -1.0); // exponent -2, mantissa -4
}

#[test]
fn refuses_bad_voltage_without_touching_bus() {
    for voltage in [f32::NAN, f32::INFINITY, 0.0, 0.99, 1.26] {
        let mut bus = Mock::new(&[]);
        assert_eq!(
            block_on(Tps546::new(&mut bus).init(voltage)),
            Err(Error::VoltageRange)
        );
        bus.done();
    }
}

#[test]
fn refuses_wrong_device_and_unsupported_voltage_mode() {
    let mut bus = Mock::new(&[
        Transaction::write(ADDRESS, std::vec![0x01, 0]),
        Transaction::write_read(ADDRESS, std::vec![0xad], std::vec![6, 0, 0, 0, 0, 0, 0]),
    ]);
    assert_eq!(
        block_on(Tps546::new(&mut bus).init(1.15)),
        Err(Error::DeviceId)
    );
    bus.done();
    let mut bus = Mock::new(&[
        Transaction::write(ADDRESS, std::vec![0x01, 0]),
        Transaction::write_read(
            ADDRESS,
            std::vec![0xad],
            std::vec![6, 0x54, 0x49, 0x54, 0x6d, 0x24, 0x41],
        ),
        Transaction::write_read(ADDRESS, std::vec![0x20], std::vec![0x40]),
    ]);
    assert_eq!(
        block_on(Tps546::new(&mut bus).init(1.15)),
        Err(Error::VoltageMode)
    );
    bus.done();
}

#[test]
fn enables_only_with_operation_readback_and_sends_shutdown() {
    let mut bus = Mock::new(&[
        Transaction::write(ADDRESS, std::vec![1, 0x80]),
        Transaction::write_read(ADDRESS, std::vec![1], std::vec![0]),
        Transaction::write(ADDRESS, std::vec![1, 0]),
    ]);
    let mut regulator = Tps546::new(&mut bus);
    assert_eq!(block_on(regulator.enable()), Err(Error::Readback));
    block_on(regulator.disable()).unwrap();
    bus.done();
}

#[test]
fn telemetry_reads_little_endian_words_and_separate_voltage_format() {
    let mut bus = Mock::new(&[
        Transaction::write_read(ADDRESS, std::vec![0x88], std::vec![0x80, 0xca]), // 5 V
        Transaction::write_read(ADDRESS, std::vec![0x8b], std::vec![0x4d, 2]),    // 589 * 2^-9 V
        Transaction::write_read(ADDRESS, std::vec![0x8c], std::vec![0x80, 0xda]), // 20 A
        Transaction::write_read(ADDRESS, std::vec![0x8d], std::vec![0x20, 0xeb]), // 100 C
        Transaction::write_read(ADDRESS, std::vec![0x79], std::vec![0x10, 0x40]),
    ]);
    let mut regulator = Tps546::new(&mut bus);
    regulator.exponent = -9;
    let data = block_on(regulator.telemetry()).unwrap();
    assert_eq!(data.vin, 5.0);
    assert!((data.vout - 1.15).abs() < 0.001);
    assert_eq!(data.iout, 20.0);
    assert_eq!(data.temperature, 100.0);
    assert_eq!(data.status, 0x4010);
    bus.done();
}

#[test]
fn profile_is_volatile_and_does_not_enable_output() {
    // Capture initialization so every register write is checked, including the absence of NVM
    // writes and OPERATION=ON. Identity, voltage mode and final readback are simulated.
    #[derive(Default)]
    struct Capture {
        writes: std::vec::Vec<std::vec::Vec<u8>>,
    }
    impl embedded_hal::i2c::ErrorType for Capture {
        type Error = core::convert::Infallible;
    }
    impl I2c for Capture {
        async fn transaction(
            &mut self,
            address: u8,
            operations: &mut [embedded_hal_async::i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
            use embedded_hal_async::i2c::Operation;
            assert_eq!(address, ADDRESS);
            match operations {
                [Operation::Write(w)] => self.writes.push(w.to_vec()),
                [Operation::Write(w), Operation::Read(r)] => match w[0] {
                    0xad => r.copy_from_slice(&[6, 0x54, 0x49, 0x54, 0x6d, 0x24, 0x41]),
                    0x20 => r[0] = 0x17, // LINEAR16 exponent -9
                    0x21 => r.copy_from_slice(&589u16.to_le_bytes()),
                    0x01 => r[0] = 0,
                    _ => panic!("unexpected read"),
                },
                _ => panic!("unexpected transaction"),
            }
            Ok(())
        }
    }
    let mut bus = Capture::default();
    block_on(Tps546::new(&mut bus).init(1.15)).unwrap();
    assert_eq!(bus.writes.first().unwrap(), &[1, 0]);
    assert!(bus
        .writes
        .iter()
        .all(|w| w[0] != 0x15 && w[0] != 0x16 && w.as_slice() != [1, 0x80]));
    assert!(bus.writes.contains(&std::vec![0x21, 0x4d, 2]));
    assert!(bus.writes.contains(&std::vec![0x24, 0x80, 2])); // VOUT_MAX 1.25 V
    assert!(bus.writes.contains(&std::vec![0x02, 0x1b])); // ignore CONTROL pin
    assert!(bus.writes.contains(&std::vec![0x47, 0xc0])); // overcurrent latches off
}
