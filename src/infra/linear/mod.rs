//! Linear: its GraphQL API ([`client`], [`error`]) and signing in ([`auth`]).

pub mod auth;
pub mod client;
#[cfg(test)]
mod decode_tests;
pub mod error;
