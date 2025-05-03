use crate::context::main_thread::{MainThreadCallback, MainThreadContext};
use bevy_ecs::{prelude::Resource, schedule::InternedScheduleLabel};
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Resource, Clone, Default)]
pub struct TaskChannels {
    channels: Arc<DashMap<InternedScheduleLabel, ChannelPair>>,
}

struct ChannelPair {
    task_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    task_rx: tokio::sync::mpsc::UnboundedReceiver<MainThreadCallback>,
}

impl Default for ChannelPair {
    fn default() -> Self {
        let (task_tx, task_rx) = tokio::sync::mpsc::unbounded_channel();
        Self { task_tx, task_rx }
    }
}

impl TaskChannels {
    pub fn submit(
        &self,
        schedule: InternedScheduleLabel,
        callback: impl FnOnce(MainThreadContext) + Send + 'static,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.task_tx(schedule).send(Box::new(callback))?;
        Ok(())
    }

    pub fn task_tx(
        &self,
        schedule: InternedScheduleLabel,
    ) -> tokio::sync::mpsc::UnboundedSender<MainThreadCallback> {
        self.channels
            .entry(schedule)
            .or_default()
            .value()
            .task_tx
            .clone()
    }

    pub fn try_recv(&self, schedule: InternedScheduleLabel) -> Option<MainThreadCallback> {
        self.channels
            .get_mut(&schedule)
            .and_then(|mut channel_pair| channel_pair.task_rx.try_recv().ok())
    }
}
