#![cfg(all(feature = "wasm", target_arch = "wasm32"))]

use bevy_app::App;
use bevy_ecs::system::SystemState;
use bevy_wasm_tasks::{Tasks, TasksPlugin};
use std::time::Duration;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(TasksPlugin::default());
    app
}

#[wasm_bindgen_test]
async fn auto_task_joins() {
    let mut app = app();
    let mut state = SystemState::<Tasks>::new(app.world_mut());
    let tasks = state.get(app.world());
    let mut task = tasks.spawn_auto(|_| async { 42 });
    assert_eq!(task.join().await, 42);
}

#[wasm_bindgen_test]
async fn wasm_task_joins() {
    let mut app = app();
    let mut state = SystemState::<Tasks>::new(app.world_mut());
    let tasks = state.get(app.world());
    let mut task = tasks.spawn_wasm(|_| async { 42 });
    assert_eq!(task.join().await, 42);
}

#[wasm_bindgen_test]
async fn dropped_handle_detaches() {
    let mut app = app();
    let (completed, receipt) = flume::bounded(1);
    let mut state = SystemState::<Tasks>::new(app.world_mut());
    let tasks = state.get(app.world());
    drop(tasks.spawn_wasm(move |_| async move {
        let _ = completed.send(42);
    }));
    let value = xrt::timeout(Duration::from_secs(1), receipt.recv_async())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(value, 42);
}

#[wasm_bindgen_test]
async fn abort_cancels() {
    let mut app = app();
    let mut state = SystemState::<Tasks>::new(app.world_mut());
    let tasks = state.get(app.world());
    let mut task = tasks.spawn_wasm(|_| std::future::pending::<()>());
    task.abort();
    assert!(task.try_join().await.unwrap_err().is_cancelled());
}
