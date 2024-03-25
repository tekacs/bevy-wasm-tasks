use bevy_ecs::{system::Resource, world::World};

use crate::context::main_thread::{MainThreadCallback, MainThreadContext};

#[derive(Resource)]
pub struct TaskChannels {
    pub task_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    pub task_rx: tokio::sync::mpsc::UnboundedReceiver<MainThreadCallback>,
}

impl TaskChannels {
    pub fn new() -> Self {
        let (task_tx, task_rx) = tokio::sync::mpsc::unbounded_channel();
        Self { task_tx, task_rx }
    }

    pub fn task_tx(&self) -> tokio::sync::mpsc::UnboundedSender<MainThreadCallback> {
        self.task_tx.clone()
    }
}

impl Default for TaskChannels {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskChannels {
    /// Execute all of the requested runnables on the main thread.
    pub(crate) fn execute_main_thread_work(&mut self, world: &mut World, current_tick: usize) {
        while let Ok(runnable) = self.task_rx.try_recv() {
            let context = MainThreadContext {
                world,
                current_tick,
            };
            runnable(context);
        }
    }
}
