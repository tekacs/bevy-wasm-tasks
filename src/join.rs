pub enum JoinHandle<T> {
    Tokio(tokio::task::JoinHandle<T>),
    RemoteHandle(Option<futures_util::future::RemoteHandle<T>>),
}

impl<T> JoinHandle<T> {
    pub async fn join(&mut self) -> T
    where
        T: 'static,
    {
        match self {
            Self::Tokio(handle) => handle.await.unwrap(),
            Self::RemoteHandle(handle) => handle.take().unwrap().await,
        }
    }
}

impl<T> Drop for JoinHandle<T> {
    /// To match Tokio behavior and make it easier to handle throwaway tasks,
    /// if a JoinHandle is dropped without the inner RemoteHandle being taken,
    /// we simply forget it so that it's able to continue to completion.
    fn drop(&mut self) {
        match self {
            Self::RemoteHandle(handle) => {
                if let Some(handle) = handle.take() {
                    handle.forget();
                }
            }
            Self::Tokio(_) => {}
        }
    }
}
