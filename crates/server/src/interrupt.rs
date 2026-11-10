//! What ends a request early (design §3). The handler arms an [`Interrupt`]
//! at the start of every request with the request's allowance
//! (`wall_guard_s`, else the handler's default), and emulation polls the
//! flag it returns. A request it ends answers by who set the flag
//! ([`StoppedBy`]): `wall_guard` for the wall guard, `interrupted` for the
//! caller. The caller
//! supplies it: natively the [`crate::Watchdog`] thread sets the flag when
//! the allowance runs out; a host that can't start a thread passes an
//! [`InterruptFlag`] and sets the flag itself, or never.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Arms a flag for each request; set means stop.
pub trait Interrupt: Send {
    /// Arms for one request allowed `after` of host time, and returns the
    /// flag the request polls: clear unless `after` is zero, so no earlier
    /// request's interrupt reaches this one.
    fn arm(&mut self, after: Duration) -> Arc<AtomicBool>;

    /// The request is answered: an interrupt now would reach nothing.
    fn disarm(&mut self);

    /// Who set the armed request's flag, asked once it is set.
    fn stopped_by(&self) -> StoppedBy;
}

/// Who ended a request early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoppedBy {
    /// The request's allowance ran out (`wall_guard_s`, else the default).
    WallGuard,
    /// The caller set the flag itself ([`InterruptFlag::flag`]).
    Caller,
}

/// An interrupt with no thread and no clock: the caller keeps
/// [`InterruptFlag::flag`] and sets it to stop the request in flight (from
/// another thread, a signal handler, a page's shared memory). The allowance
/// isn't enforced, except that a zero allowance stops a request at once.
///
/// Arming clears the flag, so a stop set before a request starts is lost:
/// set it only once the request is running.
#[derive(Clone, Debug, Default)]
pub struct InterruptFlag {
    flag: Arc<AtomicBool>,
    /// Whether the armed request's allowance was zero, so the flag was set
    /// by arming it: the wall guard's stop, not the caller's.
    expired: bool,
}

impl InterruptFlag {
    pub fn new() -> InterruptFlag {
        InterruptFlag::default()
    }

    /// The flag every request polls; setting it stops the one in flight.
    /// Every request is armed with it cleared, so set it only once the
    /// request has started.
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }
}

impl Interrupt for InterruptFlag {
    fn arm(&mut self, after: Duration) -> Arc<AtomicBool> {
        self.expired = after.is_zero();
        self.flag.store(self.expired, Ordering::Relaxed);
        self.flag.clone()
    }

    fn disarm(&mut self) {}

    fn stopped_by(&self) -> StoppedBy {
        if self.expired {
            StoppedBy::WallGuard
        } else {
            StoppedBy::Caller
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arming_clears_what_the_caller_set_before_and_a_zero_allowance_is_set() {
        let mut interrupt = InterruptFlag::new();
        let held = interrupt.flag();
        held.store(true, Ordering::Relaxed);
        let armed = interrupt.arm(Duration::from_secs(60));
        assert!(!armed.load(Ordering::Relaxed), "a stale interrupt");
        held.store(true, Ordering::Relaxed);
        assert!(
            armed.load(Ordering::Relaxed),
            "the caller's flag is the one polled"
        );
        interrupt.disarm();
        assert!(interrupt.arm(Duration::ZERO).load(Ordering::Relaxed));
        assert!(
            !interrupt
                .arm(Duration::from_secs(1))
                .load(Ordering::Relaxed)
        );
    }

    #[test]
    fn a_set_flag_is_the_callers_stop_unless_the_allowance_was_zero() {
        let mut interrupt = InterruptFlag::new();
        interrupt.arm(Duration::from_secs(60));
        interrupt.flag().store(true, Ordering::Relaxed);
        assert_eq!(interrupt.stopped_by(), StoppedBy::Caller);
        interrupt.arm(Duration::ZERO);
        assert_eq!(interrupt.stopped_by(), StoppedBy::WallGuard);
        interrupt.arm(Duration::from_secs(1));
        interrupt.flag().store(true, Ordering::Relaxed);
        assert_eq!(interrupt.stopped_by(), StoppedBy::Caller, "rearmed");
    }
}
