use core::pin::Pin;
use libp2p::core::Executor;
use std::future::Future;

pub struct TokioExecutor();

impl Executor for TokioExecutor {
    fn exec(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        tokio::spawn(future);
    }
}
