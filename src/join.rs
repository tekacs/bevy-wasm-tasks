pub enum JoinHandle<T> {
    #[cfg(feature = "tokio")]
    Tokio(tokio::task::JoinHandle<T>),
    #[cfg(feature = "wasm")]
    RemoteHandle(Option<futures_util::future::RemoteHandle<T>>),
    #[doc(hidden)]
    _Phantom(core::marker::PhantomData<T>),
}

impl<T> JoinHandle<T> {
    pub async fn join(&mut self) -> T
    where
        T: 'static,
    {
        match self {
            #[cfg(feature = "tokio")]
            Self::Tokio(handle) => handle.await.unwrap(),
            #[cfg(feature = "wasm")]
            Self::RemoteHandle(handle) => handle.take().unwrap().await,
            Self::_Phantom(_) => panic!(
                "No runtime is enabled. Enable the `tokio` or `wasm` feature to use a runtime."
            ),
        }
    }
}

impl<T> Drop for JoinHandle<T> {
    /// To match Tokio behavior and make it easier to handle throwaway tasks,
    /// if a JoinHandle is dropped without the inner RemoteHandle being taken,
    /// we simply forget it so that it's able to continue to completion.
    fn drop(&mut self) {
        match self {
            #[cfg(feature = "wasm")]
            Self::RemoteHandle(handle) => {
                if let Some(handle) = handle.take() {
                    handle.forget();
                }
            }
            #[cfg(feature = "tokio")]
            Self::Tokio(_) => {}
            Self::_Phantom(_) => {}
        }
    }
}
