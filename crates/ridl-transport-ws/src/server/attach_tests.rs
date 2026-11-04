use super::*;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef};
use std::cell::Cell;

// Each read advances logical time and supplies a pong, so heartbeat liveness
// stays healthy. EOF makes a removed attach deadline fail rather than hang.
struct Pongs<'a> {
    time: &'a Cell<u64>,
    attaching: bool,
}
impl Read for Pongs<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let n = self.time.get() + 1;
        self.time.set(n);
        if n > 4 {
            return Ok(0);
        }
        bytes[..6].copy_from_slice(&[0x8a, 0x80, 0, 0, 0, 0]);
        Ok(6)
    }
}
impl Write for Pongs<'_> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Connection for Pongs<'_> {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        if self.attaching {
            let remaining = Duration::from_secs(3 - self.time.get());
            assert!(
                timeout.unwrap() <= remaining,
                "read must not outlast attach deadline"
            );
        }
        Ok(())
    }
    fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn assert_attach_timeout(session: &mut impl Driver) {
    assert_pong_end(session, Ended::LivenessTimeout, 3);
}
pub(super) fn assert_pong_end(session: &mut impl Driver, expected: Ended, reads: u64) {
    let n = Cell::new(0);
    let origin = Instant::now();
    let mut config = Config::default();
    config.liveness_timeout = Duration::from_secs(3);
    config.poll_interval = Duration::from_secs(10);
    assert_eq!(
        run_upgraded_clock(
            &mut Pongs {
                time: &n,
                attaching: expected == Ended::LivenessTimeout
            },
            session,
            &config,
            || origin + Duration::from_secs(n.get())
        ),
        expected
    );
    assert_eq!(n.get(), reads, "attach deadline must ignore pongs");
}
#[test]
fn consuming_attach_wait_keeps_prior_pong_liveness() {
    let rt = Loopback::new(CatalogRef {
        name: "test",
        hash: CatalogHash([7; 32]),
    });
    let mut selection = Selection {
        session: None,
        select: Some(move |_: RequestedCatalog<'_>| Some(Scanning(rt))),
        frame_version: 2,
        config_refused: false,
    };
    assert_pong_end(&mut selection, Ended::Io(io::ErrorKind::UnexpectedEof), 5);
}

#[test]
fn consuming_acceptance_cancels_attach_deadline() {
    let catalog = CatalogRef {
        name: "test",
        hash: CatalogHash([7; 32]),
    };
    let rt = Loopback::new(catalog);
    let mut selection = Selection {
        session: None,
        select: Some(move |_: RequestedCatalog<'_>| Some(Scanning(rt))),
        frame_version: 2,
        config_refused: false,
    };
    let attach =
        Frame::Attach(crate::frame::Attach::new(2, 1, catalog.hash, catalog.name).unwrap());
    let mut bytes = vec![0; attach.encoded_len()];
    attach.encode(&mut bytes).unwrap();
    assert_eq!(
        selection.on_message(&bytes, Instant::now(), &mut vec![], &Config::default()),
        Step::Continue
    );
    assert_pong_end(&mut selection, Ended::Io(io::ErrorKind::UnexpectedEof), 5);
}

#[test]
fn consuming_zero_poll_interval_is_invalid_input() {
    struct Unattached(bool);
    impl Driver for Unattached {
        fn bounded_attach_wait(&self) -> bool {
            self.0
        }
        fn attached(&self) -> bool {
            false
        }
        fn on_message(&mut self, _: &[u8], _: Instant, _: &mut Vec<Vec<u8>>, _: &Config) -> Step {
            Step::Continue
        }
        fn tick(&mut self, _: Instant, _: &mut Vec<Vec<u8>>) -> Step {
            Step::Continue
        }
    }
    struct RejectZero;
    impl Read for RejectZero {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Ok(0)
        }
    }
    impl Write for RejectZero {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Connection for RejectZero {
        fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
            if t == Some(Duration::ZERO) {
                Err(io::ErrorKind::InvalidInput.into())
            } else {
                Ok(())
            }
        }
        fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
            Ok(())
        }
    }
    let mut config = Config::default();
    config.poll_interval = Duration::ZERO;
    for (bounded, error) in [
        (false, io::ErrorKind::InvalidInput),
        (true, io::ErrorKind::UnexpectedEof),
    ] {
        assert_eq!(
            run_upgraded(&mut RejectZero, &mut Unattached(bounded), &config),
            Ended::Io(error)
        );
    }
}

#[test]
fn provided_attach_deadline_checks_events_and_idle_reads_independently() {
    struct Attaching {
        attached: bool,
    }
    impl Driver for Attaching {
        fn bounded_attach_wait(&self) -> bool {
            true
        }
        fn attached(&self) -> bool {
            self.attached
        }
        fn on_message(&mut self, _: &[u8], _: Instant, _: &mut Vec<Vec<u8>>, _: &Config) -> Step {
            self.attached = true;
            Step::Continue
        }
        fn tick(&mut self, _: Instant, _: &mut Vec<Vec<u8>>) -> Step {
            Step::Continue
        }
    }
    struct AtDeadline<'a> {
        time: &'a Cell<u64>,
        binary: bool,
    }
    impl Read for AtDeadline<'_> {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            match self.time.get() {
                0 => {
                    self.time.set(2);
                    b[..6].copy_from_slice(&[0x8a, 0x80, 0, 0, 0, 0]);
                    return Ok(6);
                }
                2 => self.time.set(3),
                _ => return Ok(0),
            }
            if self.binary {
                b[..6].copy_from_slice(&[0x82, 0x80, 0, 0, 0, 0]);
                Ok(6)
            } else {
                Err(io::ErrorKind::TimedOut.into())
            }
        }
    }
    impl Write for AtDeadline<'_> {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Connection for AtDeadline<'_> {
        fn set_read_timeout(&self, _: Option<Duration>) -> io::Result<()> {
            Ok(())
        }
        fn set_write_timeout(&self, _: Option<Duration>) -> io::Result<()> {
            Ok(())
        }
    }
    for binary in [true, false] {
        let time = Cell::new(0);
        let origin = Instant::now();
        let mut config = Config::default();
        config.liveness_timeout = Duration::from_secs(3);
        assert_eq!(
            run_upgraded_clock(
                &mut AtDeadline {
                    time: &time,
                    binary
                },
                &mut Attaching { attached: false },
                &config,
                || origin + Duration::from_secs(time.get())
            ),
            Ended::LivenessTimeout,
            "binary={binary}"
        );
    }
}
