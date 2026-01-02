pub mod models;
pub mod analytics;
pub mod charts;
pub mod api;

// Conditional compilation for db module since it depends on WASM-only rexie
#[cfg(target_arch = "wasm32")]
pub mod db;
