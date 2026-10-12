//! Passenger read models: static catalogs and route-keyed service, independent of the
//! database row types they are derived from.
pub mod catalog;
pub mod service;

pub use catalog::CatalogStore;
pub use service::ServiceStore;
