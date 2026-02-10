//! Logging utilities that provide configurable durability guarantees.
//!
//! Webdis supports a legacy `log_fsync` configuration option with three modes:
//!
//! - **`"auto"`**: Preserve the default OS/filesystem persistence behavior (no explicit sync).
//! - **`N` milliseconds**: Call `fsync` at most once per `N` ms.
//! - **`"all"`**: Call `fsync` after each log write (potentially very expensive).
//!
//! The main binary uses `tracing_appender::non_blocking`, which performs log I/O on a
//! dedicated worker thread. This module wraps the worker thread's output writer and
//! performs explicit synchronization (`fsync`) according to the configured mode.

use crate::config::{LogFsync, LogFsyncMode};
use std::fs::File;
use std::io;
use std::io::Write;
use std::time::{Duration, Instant};

/// A writer that can explicitly synchronize buffered data to durable storage.
///
/// This models the `fsync(2)` behavior from POSIX. For `std::fs::File`, this is
/// implemented using [`File::sync_all`].
pub trait SyncableWriter: Write {
    /// Synchronize data and metadata to durable storage.
    fn sync_all(&mut self) -> io::Result<()>;
}

impl SyncableWriter for File {
    fn sync_all(&mut self) -> io::Result<()> {
        File::sync_all(self)
    }
}

/// Source of monotonic time used for fsync throttling.
///
/// This exists primarily to make `N ms` behavior deterministic in tests.
pub trait Clock: Send + 'static {
    /// Returns a monotonic timestamp suitable for measuring elapsed durations.
    fn now(&self) -> Instant;
}

/// A clock backed by [`Instant::now`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Debug, Clone, Copy)]
enum FsyncPolicy {
    /// No explicit synchronization.
    Auto,
    /// Synchronize after every write.
    All,
    /// Synchronize at most once per interval.
    Every(Duration),
}

impl FsyncPolicy {
    fn from_config(cfg: Option<&LogFsync>) -> Self {
        match cfg {
            None => Self::Auto,
            Some(LogFsync::Mode(LogFsyncMode::Auto)) => Self::Auto,
            Some(LogFsync::Mode(LogFsyncMode::All)) => Self::All,
            Some(LogFsync::Millis(ms)) => Self::Every(Duration::from_millis(*ms)),
        }
    }
}

/// A writer wrapper that enforces `log_fsync` semantics on an underlying writer.
///
/// `FsyncWriter` is designed to be used underneath `tracing_appender::non_blocking`.
/// In that configuration, the `FsyncWriter` lives on the worker thread, so its
/// internal state does not require synchronization primitives.
///
/// Note: In `"all"` mode, this calls `fsync` after each write. This can become a
/// severe performance bottleneck and should only be enabled when durability is
/// more important than throughput.
#[derive(Debug)]
pub struct FsyncWriter<W, C = SystemClock> {
    inner: W,
    policy: FsyncPolicy,
    last_sync: Option<Instant>,
    clock: C,
}

impl<W> FsyncWriter<W, SystemClock>
where
    W: SyncableWriter,
{
    /// Creates a new `FsyncWriter` using the system clock.
    pub fn new(inner: W, log_fsync: Option<&LogFsync>) -> Self {
        Self::with_clock(inner, log_fsync, SystemClock)
    }
}

impl<W, C> FsyncWriter<W, C>
where
    W: SyncableWriter,
    C: Clock,
{
    /// Creates a new `FsyncWriter` with an injected monotonic clock.
    pub fn with_clock(inner: W, log_fsync: Option<&LogFsync>, clock: C) -> Self {
        Self {
            inner,
            policy: FsyncPolicy::from_config(log_fsync),
            last_sync: None,
            clock,
        }
    }

    fn maybe_sync(&mut self) -> io::Result<()> {
        match self.policy {
            FsyncPolicy::Auto => Ok(()),
            FsyncPolicy::All => self.inner.sync_all(),
            FsyncPolicy::Every(interval) => {
                // "At most once per N ms": only sync when enough time has elapsed.
                // An interval of 0 ms degenerates to syncing every time.
                let now = self.clock.now();
                let should_sync = match self.last_sync {
                    None => true,
                    Some(prev) => now.saturating_duration_since(prev) >= interval,
                };
                if should_sync {
                    self.inner.sync_all()?;
                    self.last_sync = Some(now);
                }
                Ok(())
            }
        }
    }

    /// Consumes the wrapper and returns the underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W, C> Write for FsyncWriter<W, C>
where
    W: SyncableWriter,
    C: Clock,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        if n > 0 {
            self.maybe_sync()?;
        }
        Ok(n)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.write_all(buf)?;
        if !buf.is_empty() {
            self.maybe_sync()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
