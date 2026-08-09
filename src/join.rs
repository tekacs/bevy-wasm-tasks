pub struct JoinHandle<T>(pub(crate) xrt::JoinHandle<T>);

impl<T> JoinHandle<T> {
    pub async fn join(&mut self) -> T
    where
        T: 'static,
    {
        self.try_join().await.unwrap()
    }

    pub async fn try_join(&mut self) -> Result<T, xrt::JoinError>
    where
        T: 'static,
    {
        (&mut self.0).await
    }

    pub fn abort(&self) {
        self.0.abort();
    }

    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
}
