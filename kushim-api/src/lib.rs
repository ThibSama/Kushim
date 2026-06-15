pub mod auth;
pub mod config;
pub mod db;
pub mod domain;
pub mod errors;
pub mod http;
pub mod repositories;
pub mod services;
pub mod state;

// Shared test-only fixtures (e.g. the canonical authentication reference-data
// row needed by HTTP integration tests). Compiled only under `cfg(test)` so
// nothing ships into the production binary.
#[cfg(test)]
pub mod test_support;
