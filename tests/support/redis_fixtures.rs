#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn unique_key(prefix: &str) -> String {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{id}")
}
