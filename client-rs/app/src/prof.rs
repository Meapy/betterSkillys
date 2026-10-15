//! Frame timings by section, for `--bench`: `mark("name")` charges the time since the last mark
//! to that name. Off (and nearly free) unless turned on.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use web_time::Instant;

static ON: AtomicBool = AtomicBool::new(false);

struct State {
    last: Instant,
    totals: Vec<(&'static str, f64)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { last: Instant::now(), totals: Vec::new() });
}

pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

/// Start timing from now (the start of a frame).
pub fn start() {
    if ON.load(Ordering::Relaxed) {
        STATE.with(|s| s.borrow_mut().last = Instant::now());
    }
}

pub fn mark(name: &'static str) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let now = Instant::now();
        let t = (now - s.last).as_secs_f64();
        s.last = now;
        match s.totals.iter_mut().find(|e| e.0 == name) {
            Some(e) => e.1 += t,
            None => s.totals.push((name, t)),
        }
    });
}

/// Each section's average over `frames` frames, in ms, and start again.
pub fn report(frames: u32) -> String {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let out = s
            .totals
            .iter()
            .map(|(n, t)| format!("{n} {:.3}", t / frames.max(1) as f64 * 1000.0))
            .collect::<Vec<_>>()
            .join(", ");
        s.totals.clear();
        out
    })
}
