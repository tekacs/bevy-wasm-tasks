use bevy_app::{App, Plugin, Update};
use bevy_ecs::{prelude::World, system::Resource};
use futures_util::FutureExt;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use futures_util::future::RemoteHandle;

/// An internal struct keeping track of how many ticks have elapsed since the start of the program.
#[derive(Resource)]
struct UpdateTicks {
    ticks: Arc<AtomicUsize>,
    update_watch_tx: tokio::sync::watch::Sender<()>,
}

impl UpdateTicks {
    fn increment_ticks(&self) -> usize {
        let new_ticks = self.ticks.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        self.update_watch_tx
            .send(())
            .expect("Failed to send update_watch channel message");
        new_ticks
    }
}

/// The Bevy [`Plugin`] which sets up the [`WASMTasksRuntime`] Bevy resource and registers
/// the [`tick_runtime_update`] exclusive system.
pub struct WASMTasksPlugin;

impl Plugin for WASMTasksPlugin {
    fn build(&self, app: &mut App) {
        let ticks = Arc::new(AtomicUsize::new(0));
        let (update_watch_tx, update_watch_rx) = tokio::sync::watch::channel(());
        app.insert_resource(UpdateTicks {
            ticks: ticks.clone(),
            update_watch_tx,
        });
        app.insert_resource(WASMTasksRuntime::new(ticks, update_watch_rx));
        app.add_systems(Update, tick_runtime_update);
    }
}

/// The Bevy exclusive system which executes the main thread callbacks that background
/// tasks requested using [`run_on_main_thread`](TaskContext::run_on_main_thread). You
/// can control which [`CoreStage`] this system executes in by specifying a custom
/// [`tick_stage`](WASMTasksPlugin::tick_stage) value.
pub fn tick_runtime_update(world: &mut World) {
    let current_tick = {
        let tick_counter = match world.get_resource::<UpdateTicks>() {
            Some(counter) => counter,
            None => return,
        };

        // Increment update ticks and notify watchers of update tick.
        tick_counter.increment_ticks()
    };

    if let Some(mut runtime) = world.remove_resource::<WASMTasksRuntime>() {
        runtime.execute_main_thread_work(world, current_tick);
        world.insert_resource(runtime);
    }
}

type MainThreadCallback = Box<dyn FnOnce(MainThreadContext) + Send + 'static>;

/// The Bevy [`Resource`] which allows for spawning new background tasks.
#[derive(Resource)]
pub struct WASMTasksRuntime(Box<WASMTasksRuntimeInner>);

/// The inner fields are boxed to reduce the cost of the every-frame move out of and back into
/// the world in [`tick_runtime_update`].
struct WASMTasksRuntimeInner {
    ticks: Arc<AtomicUsize>,
    update_watch_rx: tokio::sync::watch::Receiver<()>,
    update_run_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    update_run_rx: tokio::sync::mpsc::UnboundedReceiver<MainThreadCallback>,
}

pub struct JoinHandle<T> {
    handle: Option<RemoteHandle<T>>,
}

impl <T> JoinHandle<T> {
    pub fn take(&mut self) -> Option<RemoteHandle<T>> {
        self.handle.take()
    }
}

impl<T> Drop for JoinHandle<T> {
    /// To match Tokio behavior and make it easier to handle throwaway tasks,
    /// if a JoinHandle is dropped without the inner RemoteHandle being taken,
    /// we simply forget it so that it's able to continue to completion.
    fn drop(&mut self) {
        if let Some(handle) = self.take() {
            handle.forget();
        }
    }
}

impl WASMTasksRuntime {
    fn new(ticks: Arc<AtomicUsize>, update_watch_rx: tokio::sync::watch::Receiver<()>) -> Self {
        let (update_run_tx, update_run_rx) = tokio::sync::mpsc::unbounded_channel();

        Self(Box::new(WASMTasksRuntimeInner {
            ticks,
            update_watch_rx,
            update_run_tx,
            update_run_rx,
        }))
    }

    /// Spawn a task which will run using WASM futures. The background task is provided a
    /// [`TaskContext`] which allows it to do things like [sleep for a given number of main thread updates](TaskContext::sleep_updates)
    /// or [invoke callbacks on the main Bevy thread](TaskContext::run_on_main_thread).
    pub fn spawn_background_task<Task, Output, Spawnable>(&self, spawnable_task: Spawnable) -> JoinHandle<Output>
    where
        Task: Future<Output = Output> + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + 'static,
    {
        let inner = &self.0;
        let context = TaskContext {
            update_watch_rx: inner.update_watch_rx.clone(),
            ticks: inner.ticks.clone(),
            update_run_tx: inner.update_run_tx.clone(),
        };
        let future = spawnable_task(context);
        let (future, handle) = future.remote_handle();
        wasm_bindgen_futures::spawn_local(future);
        JoinHandle {
            handle: Some(handle),
        }
    }

    /// Execute all of the requested runnables on the main thread.
    pub(crate) fn execute_main_thread_work(&mut self, world: &mut World, current_tick: usize) {
        while let Ok(runnable) = self.0.update_run_rx.try_recv() {
            let context = MainThreadContext {
                world,
                current_tick,
            };
            runnable(context);
        }
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

/// The context arguments which are available to background tasks spawned onto the
/// [`WASMTasksRuntime`].
#[derive(Clone)]
pub struct TaskContext {
    update_watch_rx: tokio::sync::watch::Receiver<()>,
    update_run_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    ticks: Arc<AtomicUsize>,
}

impl TaskContext {
    /// Returns the current value of the ticket count from the main thread - how many updates
    /// have occurred since the start of the program. Because the tick count is updated from the
    /// main thread, the tick count may change any time after this function call returns.
    pub fn current_tick(&self) -> usize {
        self.ticks.load(Ordering::SeqCst)
    }

    /// Sleeps the background task until a given number of main thread updates have occurred. If
    /// you instead want to sleep for a given length of wall-clock time, sleep using wasmtimer or similar
    /// function.
    pub async fn sleep_updates(&mut self, updates_to_sleep: usize) {
        let target_tick = self
            .ticks
            .load(Ordering::SeqCst)
            .wrapping_add(updates_to_sleep);
        while self.ticks.load(Ordering::SeqCst) < target_tick {
            if self.update_watch_rx.changed().await.is_err() {
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
        if self.update_run_tx.send(Box::new(move |ctx| {
            if output_tx.send(runnable(ctx)).is_err() {
                panic!("Failed to sent output from operation run on main thread back to waiting task");
            }
        })).is_err() {
            panic!("Failed to send operation to be run on main thread");
        }
        output_rx
            .await
            .expect("Failed to receive output from operation on main thread")
    }
}

