use bevy_app::{
    App, First, Last, Plugin, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update,
};
use bevy_ecs::{
    prelude::World,
    schedule::{InternedScheduleLabel, ScheduleLabel},
    system::{Res, SystemParam, SystemState},
};
use context::main_thread::MainThreadContext;
use std::future::Future;
use task_channels::TaskChannels;
use ticks::{TicksPlugin, UpdateTicks};

pub use context::main_thread::MainThreadRunConfiguration;
pub use context::task::TaskContext;
pub use join::JoinHandle;
pub use runtime::Runtime;
pub use scheduler::{Run, Scheduler};

pub mod context;
pub mod join;
pub mod runtime;
pub mod scheduler;
pub mod task_channels;
pub mod ticks;

#[derive(SystemParam)]
pub struct Tasks<'w> {
    runtime: Res<'w, Runtime>,
    task_channels: Res<'w, TaskChannels>,
    ticks: Res<'w, UpdateTicks>,
}

impl<'w> Tasks<'w> {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    #[inline(always)]
    pub fn task_context(&self) -> TaskContext {
        TaskContext {
            tick_rx: self.ticks.tick_rx(),
            task_channels: self.task_channels.clone(),
            ticks: self.ticks.ticks(),
        }
    }

    /// Spawn a task which will run using futures. The background task is provided a
    /// [`TaskContext`] which allows it to do things like [sleep for a given number of main thread updates](TaskContext::sleep_updates)
    /// or [invoke callbacks on the main Bevy thread](TaskContext::run_on_main_thread).
    #[cfg(feature = "tokio")]
    pub fn spawn_tokio<Task, Output, Spawnable>(
        &self,
        spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        let context = self.task_context();

        // Build the future first, using a precise-capture RPIT to avoid leaking
        // extra generics/lifetimes to the caller in Rust 2024.
        #[inline(always)]
        fn build<Fut, Output>(
            fut: Fut,
        ) -> impl Future<Output = Output> + Send + 'static + use<Output, Fut>
        where
            Fut: Future<Output = Output> + Send + 'static,
        {
            async move { fut.await }
        }

        let user_future = spawnable_task(context);
        let wrapper = build::<_, Output>(user_future);
        let handle = self.runtime.0.spawn(wrapper);
        JoinHandle::Tokio(handle)
    }

    #[cfg(not(feature = "tokio"))]
    fn spawn_tokio<Task, Output, Spawnable>(&self, _spawnable_task: Spawnable) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        unreachable!(
            "This function is private when the `tokio` feature is not enabled and should be uncallable."
        );
    }

    /// Spawn a task which will run using futures. The background task is provided a
    /// [`TaskContext`] which allows it to do things like [sleep for a given number of main thread updates](TaskContext::sleep_updates)
    /// or [invoke callbacks on the main Bevy thread](TaskContext::run_on_main_thread).
    #[cfg(feature = "wasm")]
    pub fn spawn_wasm<Task, Output, Spawnable>(
        &self,
        spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        use futures_util::FutureExt;
        let context = self.task_context();

        #[inline(always)]
        fn build<Fut, Output>(fut: Fut) -> impl Future<Output = Output> + 'static + use<Output, Fut>
        where
            Fut: Future<Output = Output> + 'static,
        {
            async move { fut.await }
        }

        let user_future = spawnable_task(context);
        let wrapper = build::<_, Output>(user_future);
        let (wrapper, handle) = wrapper.remote_handle();
        wasm_bindgen_futures::spawn_local(wrapper);
        JoinHandle::RemoteHandle(Some(handle))
    }

    #[cfg(not(feature = "wasm"))]
    fn spawn_wasm<Task, Output, Spawnable>(&self, _spawnable_task: Spawnable) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        unreachable!(
            "This function is private when the `wasm` feature is not enabled and should be uncallable."
        );
    }

    pub fn spawn_auto<Task, Output, Spawnable>(
        &self,
        spawnable_task: Spawnable,
    ) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        if cfg!(feature = "tokio") {
            self.spawn_tokio(spawnable_task)
        } else if cfg!(feature = "wasm") {
            self.spawn_wasm(spawnable_task)
        } else {
            panic!("No runtime is enabled. Enable the `tokio` or `wasm` feature to use a runtime.");
        }
    }
}

/// The Bevy [`Plugin`] which sets up the [`Runtime`] Bevy resource and registers
/// the [`tick_runtime_update`] exclusive system.
pub struct TasksPlugin {
    /// Callback which is used to create a Tokio runtime when the plugin is installed. The
    /// default value for this field configures a multi-threaded [`Runtime`] with IO and timer
    /// functionality enabled if building for non-wasm32 architectures. On wasm32 the current-thread
    /// scheduler is used instead.
    make_runtime: Box<dyn Fn() -> Runtime + Send + Sync + 'static>,
    /// Schedules in which to accept tasks.
    schedules: Vec<InternedScheduleLabel>,
}

impl Default for TasksPlugin {
    /// Configures the plugin to build a new Tokio [`Runtime`] with both IO and timer functionality
    /// enabled. On the wasm32 architecture, the [`Runtime`] will be the current-thread runtime, on all other
    /// architectures the [`Runtime`] will be the multi-thread runtime.
    fn default() -> Self {
        Self {
            make_runtime: Box::new(Runtime::default),
            schedules: vec![
                PreStartup.intern(),
                Startup.intern(),
                PostStartup.intern(),
                First.intern(),
                PreUpdate.intern(),
                Update.intern(),
                PostUpdate.intern(),
                Last.intern(),
            ],
        }
    }
}

impl TasksPlugin {
    /// The Bevy exclusive system which executes the main thread callbacks that background
    /// tasks requested using [`run_on_main_thread`](TaskContext::run_on_main_thread). You
    /// can control which [`CoreStage`] this system executes in by specifying a custom
    /// [`tick_stage`](TasksPlugin::tick_stage) value.
    pub fn run_tasks(schedule: impl ScheduleLabel) -> impl Fn(&mut World) {
        let schedule = schedule.intern();
        move |world: &mut World| {
            let current_tick = world.get_resource::<UpdateTicks>().unwrap().tick();
            let task_channels = world.get_resource::<TaskChannels>().unwrap().clone();
            while let Some(runnable) = task_channels.try_recv(schedule) {
                let context = MainThreadContext {
                    world,
                    current_tick,
                };
                runnable(context);
            }
        }
    }
}

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TicksPlugin)
            .init_resource::<scheduler::AsyncSystems>()
            .init_resource::<TaskChannels>()
            .insert_resource((self.make_runtime)());

        let mut system = SystemState::<Tasks>::new(app.world_mut());
        let tasks = system.get(app.world());
        let task_context = tasks.task_context();
        drop(system);
        app.insert_resource(task_context);

        for label in self.schedules.clone().into_iter() {
            app.add_systems(label, Self::run_tasks(label));
        }
    }
}
