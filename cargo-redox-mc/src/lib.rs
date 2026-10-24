//! `cargo redox-mc`: a development CLI for Redox-MC native mods.
//!
//! The crate is split into a library (used by the integration tests) and the
//! `cargo-redox-mc` binary. See `cli.rs` for the Cargo subcommand contract and
//! `commands/` for each subcommand.

pub mod cli;
pub mod commands;
pub mod error;
pub mod project;
pub mod repo;
pub mod templates;
