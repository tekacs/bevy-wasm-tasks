# bevy-wasm-tasks

A simple Bevy plugin which integrates the running of futures (including !Send futures) on WASM via wasm_bindgen_futures into a Bevy app.

This code is almost entirely based on the excellent [bevy-tokio-tasks](https://github.com/EkardNT/bevy-tokio-tasks), just adapted for wasm_bindgen_futures.

[![crates.io](https://img.shields.io/crates/v/bevy-wasm-tasks)](https://crates.io/crates/bevy-wasm-tasks) [![docs.rs](https://img.shields.io/docsrs/bevy-wasm-tasks)](https://docs.rs/bevy-wasm-tasks/latest/bevy_wasm_tasks/)

## How To

### How to initialize this plugin

To initialize the plugin, simply install the `WASMTasksPlugin` when initializing your Bevy app.

### How to spawn a background task

To spawn a background task from a Bevy system function, add a `WASMTasksRuntime` as a resource parameter and call
the `spawn_background_task` function.

```rust
fn example_system(runtime: ResMut<WASMTasksRuntime>) {
    runtime.spawn_background_task(|_ctx| async move {
        log::info!("This task is running in a WASM future");
    });
}
```

### How to synchronize with Bevy

Often times, background tasks will need to synchronize with the main Bevy app at certain points. You may do this
by calling the `run_on_main_thread` function on the `TaskContext` that is passed to each background task.

```rust
fn example_system(runtime: ResMut<WASMTasksRuntime>) {
    runtime.spawn_background_task(|mut ctx| async move {
        log::info!("This print executes from a background WASM future");
        ctx.run_on_main_thread(move |ctx| {
            // The inner context gives access to a mutable Bevy World reference.
            let _world: &mut World = ctx.world;
        }).await;
    });
}
```

## Version Compatibility

This crate's major and minor version numbers will match Bevy's. To allow this crate to publish updates
between Bevy updates, the patch version is allowed to increment independent of Bevy's release cycle.

| bevy-tokio-tasks version | bevy version | tokio version |
|---|---|---|
| 0.11.0 | 0.11.0 | 1 |
| 0.10.2 | 0.10.1 | 1 |
| 0.10.1 | 0.10.0 | 1 |
| 0.10.0 | 0.10.0 | 1 |
| 0.9.5 | 0.9.1 | 1 |
| 0.9.4 | 0.9.1 | 1 |
| 0.9.3 | 0.9.1 | 1 |
| 0.9.2 | 0.9.1 | 1 |
| 0.9.1 | 0.9.1 | 1 |
| 0.9.0 | 0.9.1 | 1 |
