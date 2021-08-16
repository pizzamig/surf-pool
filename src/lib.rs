use async_std::sync::{Mutex, MutexGuardArc};
use async_weighted_semaphore::{Semaphore, SemaphoreGuardArc};
use std::sync::Arc;
use surf::Client;
use thiserror::Error;

const MAX_POOL_SIZE: usize = 100;
pub type Result<T> = ::std::result::Result<T, SurfPoolError>;

#[derive(Debug)]
pub struct SurfPool {
    pool: Vec<Arc<Mutex<Client>>>,
    semaphore: Arc<Semaphore>,
    connect_request: Option<surf::Request>,
}

#[derive(Debug, Default)]
pub struct SurfPoolBuilder {
    size: usize,
    connect_request: Option<surf::RequestBuilder>,
    pre_connect: bool,
}

#[derive(Debug, Error)]
pub enum SurfPoolError {
    #[error("Size {0} is not valid (0 < size < {})", MAX_POOL_SIZE)]
    SizeNotValid(usize),
}

impl SurfPoolBuilder {
    pub fn new(size: usize) -> Result<Self> {
        if size == 0 || size > MAX_POOL_SIZE {
            return Err(SurfPoolError::SizeNotValid(size));
        }
        Ok(SurfPoolBuilder {
            size,
            ..Default::default()
        })
    }
    pub fn connect_request(mut self, connect_request: surf::RequestBuilder) -> Self {
        self.connect_request = Some(connect_request);
        self
    }
    pub fn pre_connect(mut self, pre_connect: bool) -> Self {
        self.pre_connect = pre_connect;
        self
    }
    pub async fn build(self) -> SurfPool {
        let mut pool = Vec::with_capacity(self.size);
        for _ in 0..self.size {
            let m = Arc::new(Mutex::new(Client::new()));
            pool.push(m.clone());
        }
        let connect_request = if let Some(req) = self.connect_request {
            let req = req.build();

            if self.pre_connect {
                for m in &pool {
                    let c = m.lock().await;
                    c.recv_bytes(req.clone()).await.unwrap_or_default();
                }
            }
            Some(req)
        } else {
            None
        };
        SurfPool {
            pool,
            semaphore: Arc::new(Semaphore::new(self.size)),
            connect_request,
        }
    }
}

#[derive(Debug)]
pub struct Handler {
    sg: SemaphoreGuardArc,
    mg: MutexGuardArc<Client>,
}

impl SurfPool {
    fn get_pool_size(&self) -> usize {
        self.pool.len()
    }
    async fn get_handler(&self) -> Option<Handler> {
        let sg = self.semaphore.acquire_arc(1).await.unwrap();
        for m in &self.pool {
            if let Some(mg) = m.try_lock_arc() {
                return Some(Handler { sg, mg });
            }
        }
        None
    }
}

impl Handler {
    pub fn get_client(&self) -> &Client {
        &*self.mg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[async_std::test]
    async fn with_pre_connected_pool() {
        let builder = SurfPoolBuilder::new(3)
            .unwrap()
            .connect_request(surf::get("https://pot.pizzamig.dev"))
            .pre_connect(true);
        let uut = builder.build().await;
        assert_eq!(uut.get_pool_size(), 3);
        let handler = uut.get_handler().await;
        assert!(handler.is_some());
        let handler = handler.unwrap();
        handler
            .get_client()
            .get("https://pot.pizzamig.dev")
            .recv_string()
            .await
            .unwrap();
        let h2 = uut.get_handler().await;
        assert!(h2.is_some());
        let h2 = h2.unwrap();
        h2.get_client()
            .get("https://pot.pizzamig.dev")
            .recv_string()
            .await
            .unwrap();
    }

    #[async_std::test]
    async fn not_pre_connected_pool() {
        let builder = SurfPoolBuilder::new(3)
            .unwrap()
            .connect_request(surf::get("https://pot.pizzamig.dev"))
            .pre_connect(false);
        let uut = builder.build().await;
        assert_eq!(uut.get_pool_size(), 3);
        let handler = uut.get_handler().await;
        assert!(handler.is_some());
        let handler = handler.unwrap();
        handler
            .get_client()
            .get("https://pot.pizzamig.dev")
            .recv_string()
            .await
            .unwrap();
        drop(handler);
        let h2 = uut.get_handler().await;
        assert!(h2.is_some());
        let h2 = h2.unwrap();
        h2.get_client()
            .get("https://pot.pizzamig.dev")
            .recv_string()
            .await
            .unwrap();
    }
}
