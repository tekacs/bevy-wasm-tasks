# bevy-wasm-tasks

A Bevy plugin which integrates the running of futures (including !Send futures) into a Bevy app.

This code was originally based on [bevy-tokio-tasks](https://github.com/EkardNT/bevy-tokio-tasks), but heavily adapted.

[![crates.io](https://img.shields.io/crates/v/bevy-wasm-tasks)](https://crates.io/crates/bevy-wasm-tasks) [![docs.rs](https://img.shields.io/docsrs/bevy-wasm-tasks)](https://docs.rs/bevy-wasm-tasks/latest/bevy_wasm_tasks/)

## Command-based async systems

This crate exposes a `Scheduler` `SystemParam` which can be used to run async work keyed by the
current system name (`SystemName`). Unlike `Tasks::spawn_auto`, `Scheduler::async_system` defers
system param acquisition + setup to the end of the current Bevy schedule by enqueuing a `Command`.

The `Run::OnChange { triggered }` mode treats `triggered == true` as "schedule a run". If the job is
already in-flight, it schedules exactly one follow-up run after completion (coalesced). If
`triggered` later becomes false, the follow-up run remains scheduled.
