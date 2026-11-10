//! The wall-clock guard (design §3), the native [`Interrupt`]: one thread
//! per handler, armed for each request, that sets the request's interrupt
//! flag if the request outlives its allowance. Emulation polls the flag; the
//! thread is joined on drop. The caller starts it (the CLI and `serve` do);
//! the handler only arms it.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::interrupt::Interrupt;

/// The default allowance: a safety net, not a budget.
pub const WALL_GUARD: Duration = Duration::from_secs(60);

/// The wall guard's thread.
pub struct Watchdog {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

#[derive(Default)]
struct State {
    /// The armed request's deadline and flag.
    armed: Option<(Instant, Arc<AtomicBool>)>,
    shutdown: bool,
}

impl Watchdog {
    /// Starts the thread; an error when the host can't start one (wasm32).
    pub fn start() -> io::Result<Watchdog> {
        let shared = Arc::new(Shared::default());
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("gbcli-wall-guard".into())
                .spawn(move || shared.watch())?
        };
        Ok(Watchdog {
            shared,
            thread: Some(thread),
        })
    }
}

impl Interrupt for Watchdog {
    /// The returned flag is set `after` from now, unless the guard is
    /// disarmed or armed again first. A fresh flag per arming, so a late
    /// firing can never reach the next request.
    fn arm(&mut self, after: Duration) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(after.is_zero()));
        let deadline = Instant::now() + after;
        self.shared.lock().armed = Some((deadline, flag.clone()));
        self.shared.wake.notify_one();
        flag
    }

    fn disarm(&mut self) {
        self.shared.lock().armed = None;
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Shared {
    /// A poisoned lock only means a panic elsewhere; the state is two plain
    /// fields that are always consistent, so carry on.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn watch(&self) {
        let mut state = self.lock();
        loop {
            if state.shutdown {
                return;
            }
            state = match &state.armed {
                None => self.wake.wait(state).unwrap_or_else(|e| e.into_inner()),
                Some((deadline, flag)) => {
                    let now = Instant::now();
                    if now >= *deadline {
                        flag.store(true, Ordering::Relaxed);
                        state.armed = None;
                        state
                    } else {
                        let wait = *deadline - now;
                        self.wake
                            .wait_timeout(state, wait)
                            .unwrap_or_else(|e| e.into_inner())
                            .0
                    }
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_is_set_after_the_allowance() {
        let mut dog = Watchdog::start().unwrap();
        let flag = dog.arm(Duration::from_millis(5));
        let start = Instant::now();
        while !flag.load(Ordering::Relaxed) {
            assert!(start.elapsed() < Duration::from_secs(5), "never fired");
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_disarmed_guard_never_sets_the_flag() {
        let mut dog = Watchdog::start().unwrap();
        let flag = dog.arm(Duration::from_millis(20));
        dog.disarm();
        std::thread::sleep(Duration::from_millis(60));
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn arming_again_leaves_the_earlier_flag_unset() {
        let mut dog = Watchdog::start().unwrap();
        let first = dog.arm(Duration::from_millis(20));
        let _second = dog.arm(Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(60));
        assert!(!first.load(Ordering::Relaxed));
    }

    #[test]
    fn a_zero_allowance_is_already_expired() {
        let mut dog = Watchdog::start().unwrap();
        assert!(dog.arm(Duration::ZERO).load(Ordering::Relaxed));
    }
}
