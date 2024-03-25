use bevy_ecs::system::Resource;

#[cfg(feature = "tokio")]
use std::{future::Future, sync::Arc};

#[cfg(feature = "tokio")]
#[derive(Resource)]
pub struct Runtime(pub Arc<tokio::runtime::Runtime>);

#[cfg(feature = "tokio")]
impl Default for Runtime {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let mut runtime = tokio::runtime::Builder::new_multi_thread();
        #[cfg(target_arch = "wasm32")]
        let mut runtime = tokio::runtime::Builder::new_current_thread();
        runtime.enable_all();
        Self(Arc::new(runtime.build().expect(
            "Failed to create Tokio runtime for background tasks",
        )))
    }
}

#[cfg(not(feature = "tokio"))]
#[derive(Resource, Default)]
pub struct Runtime;

impl Runtime {
    #[cfg(feature = "tokio")]
    pub fn raw(&self) -> &tokio::runtime::Runtime {
        &self.0
    }

    #[cfg(feature = "tokio")]
    pub fn runtime_arc(&self) -> Arc<tokio::runtime::Runtime> {
        self.0.clone()
    }

    #[cfg(feature = "tokio")]
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.0.block_on(future)
    }
}
