mod support;

#[path = "performance/mod.rs"]
mod performance;

// Match the backend executable's allocator for opt-in RSS measurements.
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
