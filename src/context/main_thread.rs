use bevy_app::{First, Last, PostUpdate, PreUpdate, Update};
use bevy_ecs::{
    schedule::{InternedScheduleLabel, ScheduleLabel},
    system::{SystemParam, SystemState},
    world::World,
};

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
/// [`run_on_main_thread`](TaskContext::run_on_main_thread).
pub struct MainThreadContext<'a> {
    /// A mutable reference to the main Bevy [World].
    pub world: &'a mut World,
    /// The current update tick in which the current main thread callback is executing.
    pub current_tick: usize,
}

impl<'a> MainThreadContext<'a> {
    pub fn run<P, F, Output>(&mut self, f: F) -> Output
    where
        P: SystemParam + 'static,
        F: FnOnce(P::Item<'_, '_>) -> Output,
        Output: Send + 'static,
    {
        let mut state = SystemState::<P>::new(self.world);
        let data = state.get_mut(self.world);
        let output = f(data);
        state.apply(self.world);
        output
    }
}
