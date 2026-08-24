//! Library target for the `foac` binary. It exists so integration tests in
//! `tests/` and doc tests can link against the crate; `src/main.rs` is the
//! only intended consumer.

pub mod auth;
pub mod github;
pub mod jira;
pub mod linear;
pub mod output;
pub mod provider;
pub mod rest;
pub mod sentry;
pub mod slack;
pub mod update;
