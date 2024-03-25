use super::main_thread::{MainThreadCallback, MainThreadContext};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// The context arguments which are available to background tasks spawned onto the
/// [`TasksRuntime`].
#[derive(Clone)]
pub struct TaskContext {
    pub tick_rx: tokio::sync::watch::Receiver<()>,
    pub task_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    pub ticks: Arc<AtomicUsize>,
}

impl TaskContext {
    /// Returns the current value of the ticket count from the main thread - how many updates
    /// have occurred since the start of the program. Because the tick count is updated from the
    /// main thread, the tick count may change any time after this function call returns.
    pub fn current_tick(&self) -> usize {
        self.ticks.load(Ordering::SeqCst)
    }

    /// Sleeps the background task until a given number of main thread updates have occurred. If
    /// you instead want to sleep for a given length of wall-clock time, sleep using tokio sleep or similar.
    /// function.
    pub async fn sleep_updates(&mut self, updates_to_sleep: usize) {
        let target_tick = self
            .ticks
            .load(Ordering::SeqCst)
            .wrapping_add(updates_to_sleep);
        while self.ticks.load(Ordering::SeqCst) < target_tick {
            if self.tick_rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// Invokes a synchronous callback on the main Bevy thread. The callback will have mutable access to the
    /// main Bevy [`World`], allowing it to update any resources or entities that it wants. The callback can
    /// report results back to the background thread by returning an output value, which will then be returned from
    /// this async function once the callback runs.
    pub async fn run_on_main_thread<Runnable, Output>(&mut self, runnable: Runnable) -> Output
    where
        Runnable: FnOnce(MainThreadContext) -> Output + Send + 'static,
        Output: Send + 'static,
    {
        let (output_tx, output_rx) = tokio::sync::oneshot::channel();
        if self.task_tx.send(Box::new(move |ctx| {
            if output_tx.send(runnable(ctx)).is_err() {
                panic!("Failed to send output from operation run on main thread back to waiting task");
            }
        })).is_err() {
            panic!("Failed to send operation to be run on main thread");
        }
        output_rx
            .await
            .expect("Failed to receive output from operation on main thread")
    }
}
