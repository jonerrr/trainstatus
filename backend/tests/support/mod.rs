#![allow(dead_code, unused_imports)]

pub mod api;
pub mod contracts;
pub mod fixtures;
#[cfg(feature = "integration-tests")]
pub mod services;
pub mod stores;
#[cfg(feature = "integration-tests")]
pub use services::TestRedis;
pub type RedisPool = bb8::Pool<bb8_redis::RedisConnectionManager>;
pub use api::*;
pub use fixtures::*;
pub use stores::*;
pub mod alerts;
#[cfg(feature = "integration-tests")]
pub mod geometry;
