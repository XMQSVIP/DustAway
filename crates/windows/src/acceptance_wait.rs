//! Shared only by development probes and test binaries; no production API.
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub enum WaitOutcome {
    Finished,
    Cancelled,
    Uncertain,
}

pub fn wait(mut busy: impl FnMut() -> bool, cancel: impl FnOnce()) -> WaitOutcome {
    bounded(
        &mut busy,
        cancel,
        Duration::from_secs(120),
        Duration::from_secs(30),
    )
}

fn bounded(
    busy: &mut impl FnMut() -> bool,
    cancel: impl FnOnce(),
    deadline: Duration,
    grace: Duration,
) -> WaitOutcome {
    let start = Instant::now();
    while busy() {
        if start.elapsed() >= deadline
            || std::env::var_os("DUSTAWAY_ACCEPTANCE_CANCEL")
                .is_some_and(|p| std::path::Path::new(&p).is_file())
        {
            cancel();
            let stopping = Instant::now();
            while busy() {
                if stopping.elapsed() >= grace {
                    return WaitOutcome::Uncertain;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            return WaitOutcome::Cancelled;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    WaitOutcome::Finished
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn deadlines_never_turn_cancel_or_unconfirmed_stop_into_success() {
        assert_eq!(
            bounded(
                &mut || false,
                || panic!("unexpected cancel"),
                Duration::ZERO,
                Duration::ZERO
            ),
            WaitOutcome::Finished
        );
        let running = Cell::new(true);
        assert_eq!(
            bounded(
                &mut || running.get(),
                || running.set(false),
                Duration::ZERO,
                Duration::ZERO
            ),
            WaitOutcome::Cancelled
        );
        assert_eq!(
            bounded(&mut || true, || {}, Duration::ZERO, Duration::ZERO),
            WaitOutcome::Uncertain
        );
    }
}
