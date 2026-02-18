use bevy_app::{First, Last, PostUpdate, PreUpdate, Update};
use bevy_ecs::{
    schedule::{InternedScheduleLabel, ScheduleLabel},
    world::World,
};

use crate::into_once_system::IntoOnceSystem;

pub type MainThreadCallback = Box<dyn FnOnce(MainThreadContext) + Send + 'static>;

pub struct MainThreadRunConfiguration {
    pub schedule: InternedScheduleLabel,
}

impl Default for MainThreadRunConfiguration {
    fn default() -> Self {
        Self {
            schedule: Update.intern(),
        }
    }
}

impl MainThreadRunConfiguration {
    pub fn new_with_schedule(schedule: impl ScheduleLabel) -> Self {
        Self::default().with_schedule(schedule)
    }

    pub fn with_schedule(mut self, schedule: impl ScheduleLabel) -> Self {
        self.schedule = schedule.intern();
        self
    }

    pub fn on_first() -> Self {
        Self::new_with_schedule(First)
    }

    pub fn on_pre_update() -> Self {
        Self::new_with_schedule(PreUpdate)
    }

    pub fn on_update() -> Self {
        Self::new_with_schedule(Update)
    }

    pub fn on_post_update() -> Self {
        Self::new_with_schedule(PostUpdate)
    }

    pub fn on_last() -> Self {
        Self::new_with_schedule(Last)
    }
}

/// The context arguments which are available to main thread callbacks requested using
/// [`run`](TaskContext::run).
pub struct MainThreadContext<'a> {
    /// A mutable reference to the main Bevy [World].
    pub world: &'a mut World,
    /// The current update tick in which the current main thread callback is executing.
    pub current_tick: usize,
}

impl<'a> MainThreadContext<'a> {
    /// Runs a Bevy one-shot system with explicit system input on the main thread.
    pub fn run_with_input<In, Marker, S, Output>(
        &mut self,
        input: In::Inner<'static>,
        system: S,
    ) -> Output
    where
        In: bevy_ecs::system::SystemInput + 'static,
        S: IntoOnceSystem<In, Output, Marker>,
    {
        system.run_once(input, self.world)
    }

    /// Runs a Bevy system directly on the main thread with no explicit generic arguments.
    pub fn run<Marker, S, Output>(&mut self, system: S) -> Output
    where
        S: IntoOnceSystem<(), Output, Marker>,
    {
        self.run_with_input::<(), Marker, _, _>((), system)
    }
}
