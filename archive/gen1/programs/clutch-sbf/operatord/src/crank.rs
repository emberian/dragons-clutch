//! The crank: auto by default, hand-cranked on request.
//!
//! The one thing the browser is allowed to influence is *pacing*.  It can
//! stop the walk between steps, advance it one step, and let it run again.
//! It cannot choose a step, reorder the plan, change a transaction, or skip a
//! refusal — the plan is what it is, and the crank only decides when the next
//! turn happens.
//!
//! That boundary is the whole design in miniature: an operator bench should
//! let you slow the machine down enough to read it, and should not let the
//! reading surface become an authoring surface.

use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// Shared pacing state between the daemon's walk thread and its request
/// threads.
pub struct Crank {
    inner: Mutex<Pacing>,
    signal: Condvar,
}

#[derive(Clone, Copy)]
struct Pacing {
    auto: bool,
    /// Single steps granted while paused, consumed one per turn.
    granted: usize,
}

/// How the walk got its turn, which the bench shows so a hand-cranked run is
/// never mistaken for an automatic one.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Turn {
    Auto,
    HandCranked,
}

impl Turn {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::HandCranked => "hand-cranked",
        }
    }
}

impl Crank {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Pacing {
                auto: true,
                granted: 0,
            }),
            signal: Condvar::new(),
        })
    }

    /// Block until the walk may take its next step.
    pub fn await_turn(&self) -> Turn {
        let mut pacing = self.lock();
        loop {
            if pacing.auto {
                return Turn::Auto;
            }
            if pacing.granted > 0 {
                pacing.granted -= 1;
                return Turn::HandCranked;
            }
            pacing = self
                .signal
                .wait(pacing)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    pub fn set_auto(&self, auto: bool) {
        self.lock().auto = auto;
        self.signal.notify_all();
    }

    /// Grant one step while paused.  Ignored while running automatically,
    /// where every step is already granted.
    pub fn grant(&self) {
        let mut pacing = self.lock();
        if !pacing.auto {
            pacing.granted += 1;
        }
        drop(pacing);
        self.signal.notify_all();
    }

    /// `(auto, granted)`, for the status the bench renders.
    pub fn snapshot(&self) -> (bool, usize) {
        let pacing = *self.lock();
        (pacing.auto, pacing.granted)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Pacing> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn the_default_crank_is_automatic() {
        let crank = Crank::new();
        assert_eq!(crank.snapshot(), (true, 0));
        assert!(crank.await_turn() == Turn::Auto);
    }

    #[test]
    fn a_paused_crank_releases_exactly_one_step_per_grant() {
        let crank = Crank::new();
        crank.set_auto(false);
        crank.grant();
        assert!(crank.await_turn() == Turn::HandCranked);
        assert_eq!(crank.snapshot(), (false, 0));
    }

    #[test]
    fn resuming_releases_a_blocked_walk() {
        let crank = Crank::new();
        crank.set_auto(false);
        let waiter = Arc::clone(&crank);
        let handle = thread::spawn(move || waiter.await_turn());
        thread::sleep(Duration::from_millis(50));
        crank.set_auto(true);
        assert!(handle.join().expect("the walk thread resumes") == Turn::Auto);
    }

    #[test]
    fn granting_while_automatic_does_not_bank_steps() {
        let crank = Crank::new();
        crank.grant();
        assert_eq!(crank.snapshot(), (true, 0));
    }
}
