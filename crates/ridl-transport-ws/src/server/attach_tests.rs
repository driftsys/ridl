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
fn consuming_attach_wait_is_bounded_despite_pongs() {
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
    assert_attach_timeout(&mut selection);
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
