use bevy_ecs::resource::Resource;

#[cfg(all(feature = "tokio", not(target_arch = "wasm32")))]
use std::{future::Future, sync::Arc};

#[cfg(any(feature = "tokio", feature = "wasm"))]
#[derive(Resource, Default)]
pub struct Runtime(pub xrt::Runtime);

#[cfg(not(any(feature = "tokio", feature = "wasm")))]
#[derive(Resource, Default)]
pub struct Runtime;

impl Runtime {
    #[cfg(all(feature = "tokio", not(target_arch = "wasm32")))]
    pub fn from_tokio(runtime: Arc<tokio::runtime::Runtime>) -> Self {
        Self(xrt::Runtime::from_tokio(runtime))
    }

    #[cfg(all(feature = "tokio", not(target_arch = "wasm32")))]
    pub fn raw(&self) -> &tokio::runtime::Runtime {
        self.0.tokio()
    }

    #[cfg(all(feature = "tokio", not(target_arch = "wasm32")))]
    pub fn runtime_arc(&self) -> Arc<tokio::runtime::Runtime> {
        self.0.tokio_arc()
    }

    #[cfg(all(feature = "tokio", not(target_arch = "wasm32")))]
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.0.block_on(future)
    }
}
