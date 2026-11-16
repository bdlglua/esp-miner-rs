//! Runs the same chain operations as firmware with the supplied BM1370 driver on the host.
use bm1370::BM1370;
use bm13xx_chain::{Baud, Chain};
use bm13xx_protocol::response::{Response, ResponseType};
use futures::executor::block_on;
use std::{cell::RefCell, collections::VecDeque, convert::Infallible, rc::Rc};

#[derive(Default)]
struct Serial {
    rx: VecDeque<u8>,
    tx: Vec<Vec<u8>>,
    baud: u32,
}
struct Uart(Rc<RefCell<Serial>>);
impl embedded_io::ErrorType for Uart {
    type Error = Infallible;
}
impl embedded_io::ReadReady for Uart {
    fn read_ready(&mut self) -> Result<bool, Infallible> {
        Ok(!self.0.borrow().rx.is_empty())
    }
}
impl embedded_io_async::Read for Uart {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Infallible> {
        let mut state = self.0.borrow_mut();
        let count = buf.len().min(state.rx.len());
        for b in &mut buf[..count] {
            *b = state.rx.pop_front().unwrap();
        }
        Ok(count)
    }
}
impl embedded_io_async::Write for Uart {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        self.0.borrow_mut().tx.push(buf.to_vec());
        Ok(buf.len())
    }
    async fn flush(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}
impl Baud for Uart {
    fn set_baudrate(&mut self, baud: u32) {
        self.0.borrow_mut().baud = baud;
    }
}
struct Pin;
impl embedded_hal::digital::ErrorType for Pin {
    type Error = Infallible;
}
impl embedded_hal::digital::OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}
struct Delay;
impl embedded_hal_async::delay::DelayNs for Delay {
    async fn delay_ns(&mut self, _: u32) {}
}

#[test]
fn gamma_enumerates_initializes_and_wraps_job_ids_without_losing_responses() {
    block_on(async {
        let mut chip_id = [0xaa, 0x55, 0x13, 0x70, 0, 0, 0, 0, 0];
        chip_id[8] = (0..32)
            .find(|crc| {
                chip_id[8] = *crc;
                Response::parse(&chip_id, 16).is_ok()
            })
            .unwrap();
        let state = Rc::new(RefCell::new(Serial::default()));
        state.borrow_mut().rx.extend(chip_id);
        let mut chain = Chain::enumerate(BM1370::default(), Uart(state.clone()), Pin, Pin, Delay)
            .await
            .unwrap();
        assert_eq!(chain.asic_cnt, 1);
        chain.init(256).await.unwrap();
        chain.change_baudrate(1_000_000).await.unwrap();
        chain.reset_all_cores().await.unwrap();
        chain
            .set_hash_freq(fugit::HertzU64::MHz(525))
            .await
            .unwrap();
        chain.enable_version_rolling(0x1fff_e000).await.unwrap();
        assert_eq!(state.borrow().baud, 1_000_000);
        assert!((chain.theoretical_hashrate_ghs() - 1071.0).abs() < 1.0);
        for n in 1..=33 {
            let id = chain
                .send_job(0x2000_0000, [0x11; 32], [0x22; 32], 0x17034219, 0x6659dc4d)
                .await
                .unwrap();
            assert_eq!(id, n % 16);
            let tx = state.borrow();
            let frame = tx.tx.last().unwrap();
            assert_eq!(frame.len(), 88);
            assert_eq!(frame[4], id << 3);
            assert!(frame[4] < 0x80);
        }
        // Supplied BM1370 wire fixture: job 5, small core 14, rolled version 0x5c000.
        state.borrow_mut().rx.extend([
            0xaa, 0x55, 0x07, 0x35, 0xcd, 0xcf, 0x02, 0x5e, 0x00, 0x2e, 0x96,
        ]);
        assert!(chain.poll_response().await.unwrap().is_none()); // first poll fills RX buffer
        let resp = chain.poll_response().await.unwrap().unwrap();
        match resp {
            ResponseType::JobVer(job) => {
                assert_eq!(job.job_id, 5);
                assert_eq!(job.nonce, 0xcfcd3507);
                assert_eq!(job.small_core_id, 14);
                assert_eq!(job.version_bit, 0x0005_c000);
            }
            _ => panic!("expected version-rolling nonce"),
        }
    });
}

#[test]
fn bm1370_response_requires_sixteen_small_cores() {
    let frame = [
        0xaa, 0x55, 0x07, 0x35, 0xcd, 0xcf, 0x02, 0x5e, 0x00, 0x2e, 0x96,
    ];
    let ResponseType::JobVer(job) = Response::parse_version(&frame, 16).unwrap() else {
        panic!()
    };
    assert_eq!((job.job_id, job.small_core_id), (5, 14));
}
