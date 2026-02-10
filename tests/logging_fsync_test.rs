//! Behavioral tests for `log_fsync` durability policies.
//!
//! These are integration-style tests to ensure the fsync logic stays out of `src/`.

use std::io;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use webdis::config::{LogFsync, LogFsyncMode};
use webdis::logging::{Clock, FsyncWriter, SyncableWriter};

#[derive(Debug, Default)]
struct RecordingWriter {
    _inner: Vec<u8>,
    sync_calls: usize,
}

impl Write for RecordingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self._inner.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl SyncableWriter for RecordingWriter {
    fn sync_all(&mut self) -> io::Result<()> {
        self.sync_calls += 1;
        Ok(())
    }
}

#[derive(Clone)]
struct ManualClock {
    start: Instant,
    offset: Arc<Mutex<Duration>>,
}

impl ManualClock {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            offset: Arc::new(Mutex::new(Duration::from_millis(0))),
        }
    }

    fn advance_ms(&self, ms: u64) {
        let mut guard = self.offset.lock().unwrap();
        *guard += Duration::from_millis(ms);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        let guard = self.offset.lock().unwrap();
        self.start + *guard
    }
}

#[test]
fn fsync_all_syncs_after_each_write_all() {
    let clock = ManualClock::new();
    let writer = RecordingWriter::default();
    let log_fsync = LogFsync::Mode(LogFsyncMode::All);
    let mut fsync = FsyncWriter::with_clock(writer, Some(&log_fsync), clock);

    fsync.write_all(b"a").unwrap();
    fsync.write_all(b"b").unwrap();
    fsync.write_all(b"c").unwrap();

    let inner = fsync.into_inner();
    assert_eq!(inner.sync_calls, 3);
}

#[test]
fn fsync_every_n_ms_throttles_syncs() {
    let clock = ManualClock::new();
    let writer = RecordingWriter::default();
    let log_fsync = LogFsync::Millis(10);
    let mut fsync = FsyncWriter::with_clock(writer, Some(&log_fsync), clock.clone());

    // First write should sync immediately.
    fsync.write_all(b"first").unwrap();

    // Within interval, should not sync again.
    fsync.write_all(b"second").unwrap();
    fsync.write_all(b"third").unwrap();

    // After interval, should sync again.
    clock.advance_ms(10);
    fsync.write_all(b"fourth").unwrap();

    let inner = fsync.into_inner();
    assert_eq!(inner.sync_calls, 2);
}
