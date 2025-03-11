use super::main_thread::{MainThreadContext, MainThreadRunConfiguration};
use crate::task_channels::TaskChannels;
use bevy_ecs::resource::Resource;
use flume::Receiver;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// The context arguments which are available to background tasks spawned onto the
/// [`TasksRuntime`].
#[derive(Resource, Clone)]
pub struct TaskContext {
    pub tick_rx: tokio::sync::watch::Receiver<()>,
    pub task_channels: TaskChannels,
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
    pub async fn sleep_updates(&self, updates_to_sleep: usize) {
        let mut tick_rx = self.tick_rx.clone();
        let target_tick = self
            .ticks
            .load(Ordering::SeqCst)
            .wrapping_add(updates_to_sleep);
        while self.ticks.load(Ordering::SeqCst) < target_tick {
            if tick_rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// Invokes a synchronous callback on the main Bevy thread. The callback will have mutable access to the
    /// main Bevy [`World`], allowing it to update any resources or entities that it wants. The callback can
    /// report results back to the background thread by returning an output value, which will then be returned from
    /// this async function once the callback runs.
    pub fn submit_on_main_thread_with_config<Runnable, Output>(
        &self,
        runnable: Runnable,
        config: MainThreadRunConfiguration,
    ) -> Receiver<Output>
    where
        Runnable: FnOnce(MainThreadContext) -> Output + Send + 'static,
        Output: Send + 'static,
    {
        let (output_tx, output_rx) = flume::bounded(1);
        if self
            .task_channels
            .submit(config.schedule, move |ctx| {
                // Allow the sender to drop the output receipt channel.
                let _ = output_tx.send(runnable(ctx));
            })
            .is_err()
        {
            panic!("Failed to send operation to be run on main thread");
        }
        output_rx
    }

    /// Invokes a synchronous callback on the main Bevy thread. The callback will have mutable access to the
    /// main Bevy [`World`], allowing it to update any resources or entities that it wants. The callback can
    /// report results back to the background thread by returning an output value, which will be returned on
    /// the output channel returned from this function.
    pub fn submit_on_main_thread<Runnable, Output>(&self, runnable: Runnable) -> Receiver<Output>
    where
        Runnable: FnOnce(MainThreadContext) -> Output + Send + 'static,
        Output: Send + 'static,
    {
        self.submit_on_main_thread_with_config(runnable, Default::default())
    }

    /// Invokes a synchronous callback on the main Bevy thread. The callback will have mutable access to the
    /// main Bevy [`World`], allowing it to update any resources or entities that it wants. The callback can
    /// report results back to the background thread by returning an output value, which will then be returned from
    /// this async function once the callback runs.
    pub async fn run_on_main_thread_with_config<Runnable, Output>(
        &self,
        runnable: Runnable,
        config: MainThreadRunConfiguration,
    ) -> Output
    where
        Runnable: FnOnce(MainThreadContext) -> Output + Send + 'static,
        Output: Send + 'static,
    {
        let (output_tx, output_rx) = tokio::sync::oneshot::channel();
        if self.task_channels.submit(config.schedule,
            move |ctx| {
                if output_tx.send(runnable(ctx)).is_err() {
                    panic!(
                        "Failed to send output from operation run on main thread back to waiting task"
                    );
                }
            }
        ).is_err() {
            panic!("Failed to send operation to be run on main thread");
        }
        output_rx
            .await
            .expect("Failed to receive output from operation on main thread")
    }

    /// Invokes a synchronous callback on the main Bevy thread. The callback will have mutable access to the
    /// main Bevy [`World`], allowing it to update any resources or entities that it wants. The callback can
    /// report results back to the background thread by returning an output value, which will then be returned from
    /// this async function once the callback runs.
    pub async fn run_on_main_thread<Runnable, Output>(&self, runnable: Runnable) -> Output
    where
        Runnable: FnOnce(MainThreadContext) -> Output + Send + 'static,
        Output: Send + 'static,
    {
        self.run_on_main_thread_with_config(runnable, Default::default())
            .await
    }
}
