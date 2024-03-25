use futures_util::future::RemoteHandle;

pub struct JoinHandle<T> {
    handle: Option<RemoteHandle<T>>,
}

impl<T> JoinHandle<T> {
    pub fn new(handle: RemoteHandle<T>) -> Self {
        Self {
            handle: Some(handle),
        }
    }

    pub fn take(&mut self) -> Option<RemoteHandle<T>> {
        self.handle.take()
    }
}

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
