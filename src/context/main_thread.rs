use bevy_ecs::world::World;

pub type MainThreadCallback = Box<dyn FnOnce(MainThreadContext) + Send + 'static>;

/// The context arguments which are available to main thread callbacks requested using
/// [`run_on_main_thread`](TaskContext::run_on_main_thread).
pub struct MainThreadContext<'a> {
    /// A mutable reference to the main Bevy [World].
    pub world: &'a mut World,
    /// The current update tick in which the current main thread callback is executing.
    pub current_tick: usize,
}
