# bevy-wasm-tasks

A Bevy plugin which integrates asynchronous tasks and systems into a Bevy app across native and browser WebAssembly targets.

Task execution comes from [`cross-runtime`](https://crates.io/crates/cross-runtime): Tokio on native targets and `web-task` in browsers. `Tasks::spawn_auto` accepts portable `Send` work; `Tasks::spawn_wasm` accepts `!Send` browser work pinned to its current thread. Both return detach-on-drop handles with join, cancellation, and completion inspection.

This code was originally based on [bevy-tokio-tasks](https://github.com/EkardNT/bevy-tokio-tasks), but heavily adapted.

[![crates.io](https://img.shields.io/crates/v/bevy-wasm-tasks)](https://crates.io/crates/bevy-wasm-tasks) [![docs.rs](https://img.shields.io/docsrs/bevy-wasm-tasks)](https://docs.rs/bevy-wasm-tasks/latest/bevy_wasm_tasks/)

## Command-based async systems

This crate exposes a `Scheduler` `SystemParam` which can be used to run async work keyed by the
current system name (`SystemName`). Unlike `Tasks::spawn_auto`, `Scheduler::async_system` defers
system param acquisition + setup to the end of the current Bevy schedule by enqueuing a `Command`.

The `Run::OnChange { triggered }` mode treats `triggered == true` as "schedule a run". If the job is
already in-flight, it schedules exactly one follow-up run after completion (coalesced). If
`triggered` later becomes false, the follow-up run remains scheduled.

Run modes:

- `Run::AsOftenAsPossible`: coalescing fire-asap behavior.
- `Run::MaxRate(duration)`: coalescing fire-asap with rate limit.
- `Run::OnChange { triggered }`: edge-triggered with one queued rerun.
- `Run::Once`: execute once and never schedule again.
- `Run::Daemon`: restart on the next tick whenever the task exits; exit/error is printed to stderr.

## Browser tests

Run the wasm suite inside a real headless Chrome instance through ChromeDriver:

```sh
wasm-pack test --headless --chrome -- --features wasm
```

This executes the `#[wasm_bindgen_test]` cases for Bevy task joining, detach-on-drop, and cancellation. `cargo test --target wasm32-unknown-unknown --no-run` is only a compile gate.
