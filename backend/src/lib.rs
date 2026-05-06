//! Library crate for the backend. Exposed alongside the binary so that
//! integration tests under `tests/` can drive the real router and handlers.

pub mod config;
pub mod db;
pub mod error;
pub mod extractors;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod state;
pub mod test_support;
