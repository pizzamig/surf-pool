//! Connection pool for Surf
use async_std::sync::{Mutex, MutexGuardArc};
use async_weighted_semaphore::{Semaphore, SemaphoreGuardArc};
use std::sync::Arc;
use surf::Client;
use thiserror::Error;

const MAX_POOL_SIZE: usize = 100;
/// Convenient Result redefinition that uses [SurfPoolError] as Error
pub type Result<T> = ::std::result::Result<T, SurfPoolError>;

#[derive(Clone, Debug)]
/// The main struct, used to get a valid connection
pub struct SurfPool {
    pool: Vec<Arc<Mutex<Client>>>,
    semaphore: Arc<Semaphore>,
    health_check: Option<surf::Request>,
}

/// The builder struct, used to create a SurfPool
#[derive(Debug, Default)]
pub struct SurfPoolBuilder {
    size: usize,
    health_check: Option<surf::RequestBuilder>,
    pre_connect: bool,
}

#[derive(Debug, Error)]
pub enum SurfPoolError {
    #[error("Size {0} is not valid (0 < size < {})", MAX_POOL_SIZE)]
    SizeNotValid(usize),
}

impl SurfPoolBuilder {
    /// This function is used to create a new builder
    /// The parameter size is checked if is a valid and reasonable number
    /// It cannot be 0 or bigger than 100
    ///
    /// ```rust
    /// use surf_pool::SurfPoolBuilder;
    ///
    /// SurfPoolBuilder::new(3).unwrap();
    /// ```
    pub fn new(size: usize) -> Result<Self> {
        if size == 0 || size > MAX_POOL_SIZE {
            return Err(SurfPoolError::SizeNotValid(size));
        }
        Ok(SurfPoolBuilder {
            size,
            ..Default::default()
        })
    }
    /// The health_check is a URL used to manage the connection
    /// It's used to check the connection health status, as keepalive and
    /// as pre-connect URL
    ///
    /// ```rust
    /// use surf_pool::SurfPoolBuilder;
    ///
    /// let builder = SurfPoolBuilder::new(3)
    ///     .unwrap()
    ///     .health_check(surf::get("https://httpbin.org"));
    /// ```
    pub fn health_check(mut self, health_check: surf::RequestBuilder) -> Self {
        self.health_check = Some(health_check);
        self
    }
    /// If true, the connections are established during the build phase, using
    /// the health_check. If the health_check is not defined, the pre-connection
    /// cannot be peformed, hence it will be ignored
    ///
    /// ```rust
    /// use surf_pool::SurfPoolBuilder;
    ///
    /// let builder = SurfPoolBuilder::new(3).
    ///     unwrap()
    ///     .health_check(surf::get("https://httpbin.org"))
    ///     .pre_connect(true);
    /// ```
    pub fn pre_connect(mut self, pre_connect: bool) -> Self {
        self.pre_connect = pre_connect;
        self
    }
    /// The build function that creates the @SurfPool
    /// If a health_check is available and pre_connect is set to true
    /// the connections are established in this function
    ///
    /// ```rust
    /// use surf_pool::SurfPoolBuilder;
    ///
    /// let builder = SurfPoolBuilder::new(3).
    ///     unwrap()
    ///     .health_check(surf::get("https://httpbin.org"))
    ///     .pre_connect(true);
    /// let pool = builder.build();
    /// ```
    pub async fn build(self) -> SurfPool {
        let mut pool = Vec::with_capacity(self.size);
        for _ in 0..self.size {
            let m = Arc::new(Mutex::new(Client::new()));
            pool.push(m.clone());
        }
        let health_check = if let Some(req) = self.health_check {
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
            health_check,
        }
    }
}

#[derive(Debug)]
pub struct Handler {
    sg: SemaphoreGuardArc,
    mg: MutexGuardArc<Client>,
}

impl SurfPool {
    pub fn get_pool_size(&self) -> usize {
        self.pool.len()
    }
    /// This function return an handler representing a potential connection
    /// available in the pool.
    /// The handler is not a connection, but a Surf client can be obtained
    /// via [`get_client`]
    /// If the pool is empty, the function will wait until an handler is
    /// available again
    /// To not starve other clients, it's important to drop the handler after
    /// it has been used
    /// The return type is an [`Option`], but it should never return `None`,
    /// the system is designed in a way that, once unblocked, at least one
    /// resources should be available
    /// ```rust
    /// # futures_lite::future::block_on( async {
    ///
    /// use surf_pool::SurfPoolBuilder;
    ///
    /// let builder = SurfPoolBuilder::new(3).unwrap();
    /// let pool = builder.build().await;
    /// let handler = pool.get_handler().await;
    /// # } )
    /// ```
    pub async fn get_handler(&self) -> Handler {
        self.get_handler_option().await.unwrap()
    }

    async fn get_handler_option(&self) -> Option<Handler> {
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
    /// This function allows you to get a Surf client that can be used
    /// to perform an async http call
    /// If the connection is previously established, the connection is
    /// already ready to use
    /// ```rust
    /// # futures_lite::future::block_on( async {
    ///
    /// use surf_pool::SurfPoolBuilder;
    ///
    /// let builder = SurfPoolBuilder::new(3).unwrap();
    /// let pool = builder.build().await;
    /// let handler = pool.get_handler().await;
    /// handler
    ///     .get_client()
    ///     .get("https://httpbin.org")
    ///     .recv_string()
    ///     .await;
    /// # } )
    /// ```
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
            .health_check(surf::get("https://pot.pizzamig.dev"))
            .pre_connect(true);
        let uut = builder.build().await;
        assert_eq!(uut.get_pool_size(), 3);
        let handler = uut.get_handler().await;
        handler
            .get_client()
            .get("https://pot.pizzamig.dev")
            .recv_string()
            .await
            .unwrap();
        let h2 = uut.get_handler().await;
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
            .health_check(surf::get("https://pot.pizzamig.dev"))
            .pre_connect(false);
        let uut = builder.build().await;
        assert_eq!(uut.get_pool_size(), 3);
        let handler = uut.get_handler().await;
        handler
            .get_client()
            .get("https://pot.pizzamig.dev")
            .recv_string()
            .await
            .unwrap();
        drop(handler);
        let h2 = uut.get_handler().await;
        h2.get_client()
            .get("https://pot.pizzamig.dev")
            .recv_string()
            .await
            .unwrap();
    }
}
