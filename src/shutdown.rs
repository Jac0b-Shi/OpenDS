//! A confirmed Windows session end has one shared cleanup budget.
use std::{
    sync::OnceLock,
    thread::JoinHandle,
    time::{Duration, Instant},
};

const BUDGET: Duration = Duration::from_millis(3500);
static DEADLINE: OnceLock<Instant> = OnceLock::new();

pub fn begin() {
    DEADLINE.get_or_init(|| Instant::now() + BUDGET);
}
pub fn ending() -> bool {
    DEADLINE.get().is_some()
}
pub fn join<T>(worker: JoinHandle<T>) -> Option<std::thread::Result<T>> {
    join_until(worker, DEADLINE.get().copied())
}
fn join_until<T>(
    worker: JoinHandle<T>,
    deadline: Option<Instant>,
) -> Option<std::thread::Result<T>> {
    if let Some(deadline) = deadline {
        while !worker.is_finished() {
            if Instant::now() >= deadline {
                // Only during confirmed OS shutdown: the OS terminates remaining
                // threads. The HidHide journal remains recoverable on next launch.
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Some(worker.join())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_stalled_worker_cannot_extend_the_shared_shutdown_deadline() {
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || receive.recv().unwrap());
        let started = Instant::now();
        assert!(join_until(worker, Some(started + Duration::from_millis(30))).is_none());
        assert!(started.elapsed() < Duration::from_secs(1));
        send.send(()).unwrap();
        assert_eq!(
            join_until(std::thread::spawn(|| 42), None)
                .unwrap()
                .unwrap(),
            42
        );
    }
}
