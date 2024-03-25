pub enum JoinHandle<T> {
    Tokio(tokio::task::JoinHandle<T>),
    RemoteHandle(Option<futures_util::future::RemoteHandle<T>>),
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
