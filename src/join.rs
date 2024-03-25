pub struct JoinHandle<T> {
    #[cfg(feature = "tokio")]
    pub handle: tokio::task::JoinHandle<T>,
    #[cfg(not(feature = "tokio"))]
    pub handle: Option<futures_util::future::RemoteHandle<T>>,
}

impl<T> JoinHandle<T> {
    #[cfg(not(feature = "tokio"))]
    pub fn take(&mut self) -> Option<futures_util::future::RemoteHandle<T>> {
        self.handle.take()
    }
}

#[cfg(not(feature = "tokio"))]
impl<T> Drop for JoinHandle<T> {
    /// To match Tokio behavior and make it easier to handle throwaway tasks,
    /// if a JoinHandle is dropped without the inner RemoteHandle being taken,
    /// we simply forget it so that it's able to continue to completion.
    fn drop(&mut self) {
        if let Some(handle) = self.take() {
            handle.forget();
        }
    }
}
