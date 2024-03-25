use bevy_ecs::system::Resource;
use std::{ops::Deref, sync::Arc};

#[derive(Resource)]
pub struct TasksRuntime(Arc<Runtime>);

impl TasksRuntime {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self(runtime)
    }
}

impl Deref for TasksRuntime {
    type Target = Arc<Runtime>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "tokio")]
pub struct Runtime(pub tokio::runtime::Runtime);

#[cfg(feature = "tokio")]
impl Default for Runtime {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let mut runtime = tokio::runtime::Builder::new_multi_thread();
        #[cfg(target_arch = "wasm32")]
        let mut runtime = tokio::runtime::Builder::new_current_thread();
        runtime.enable_all();
        Self(
            runtime
                .build()
                .expect("Failed to create Tokio runtime for background tasks"),
        )
    }
}

#[cfg(not(feature = "tokio"))]
#[derive(Default)]
pub struct Runtime;
