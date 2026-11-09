//! The wall-clock guard (design §3): one thread per handler, armed for each
//! request, that sets the request's interrupt flag if the request outlives
//! its allowance. Emulation polls the flag; the thread is joined on drop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The default allowance: a safety net, not a budget.
pub const WALL_GUARD: Duration = Duration::from_secs(60);

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

/// One request's arming; dropping it disarms the guard.
pub struct Armed {
    shared: Arc<Shared>,
    flag: Arc<AtomicBool>,
}

impl Watchdog {
    pub fn new() -> Watchdog {
        let shared = Arc::new(Shared::default());
        // WEB SPIKE CUT: wasm32 can't spawn a thread ("Not supported"), so
        // there the guard never fires: only a zero allowance interrupts.
        if cfg!(target_family = "wasm") {
            return Watchdog {
                shared,
                thread: None,
            };
        }
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("gbcli-wall-guard".into())
                .spawn(move || shared.watch())
                .expect("spawn the wall-guard thread")
        };
        Watchdog {
            shared,
            thread: Some(thread),
        }
    }

    /// Arms the guard: the returned flag is set `after` from now, unless the
    /// arming is dropped first. A fresh flag per arming, so a late firing can
    /// never reach the next request.
    pub fn arm(&self, after: Duration) -> Armed {
        let flag = Arc::new(AtomicBool::new(after.is_zero()));
        let deadline = Instant::now() + after;
        self.shared.lock().armed = Some((deadline, flag.clone()));
        self.shared.wake.notify_one();
        Armed {
            shared: self.shared.clone(),
            flag,
        }
    }
}

impl Default for Watchdog {
    fn default() -> Self {
        Watchdog::new()
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

impl Armed {
    /// The flag a run polls as its interrupt.
    pub fn flag(&self) -> &Arc<AtomicBool> {
        &self.flag
    }
}

impl Drop for Armed {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        if state
            .armed
            .as_ref()
            .is_some_and(|(_, flag)| Arc::ptr_eq(flag, &self.flag))
        {
            state.armed = None;
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
        let dog = Watchdog::new();
        let armed = dog.arm(Duration::from_millis(5));
        let start = Instant::now();
        while !armed.flag().load(Ordering::Relaxed) {
            assert!(start.elapsed() < Duration::from_secs(5), "never fired");
            std::thread::yield_now();
        }
    }

    #[test]
    fn a_disarmed_guard_never_sets_the_flag() {
        let dog = Watchdog::new();
        let armed = dog.arm(Duration::from_millis(20));
        let flag = armed.flag().clone();
        drop(armed);
        std::thread::sleep(Duration::from_millis(60));
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn a_zero_allowance_is_already_expired() {
        let dog = Watchdog::new();
        assert!(dog.arm(Duration::ZERO).flag().load(Ordering::Relaxed));
    }
}
