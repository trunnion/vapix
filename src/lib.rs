//! # `axis`

#![forbid(unsafe_code)]
//#![forbid(missing_docs)]
#![forbid(unused_variables)]

mod client;
mod error;
mod transport;

pub mod v3;
pub mod v4;

pub use client::Client;
pub use error::Error;
pub(crate) use error::ResultExt;
pub use transport::Transport;

#[cfg(all(feature = "hyper"))]
pub mod hyper;

#[cfg(all(feature = "hyper"))]
pub use self::hyper::HyperTransport;

// Test support:
#[cfg(test)]
pub(crate) mod test_utils;

#[cfg(test)]
pub(crate) use test_utils::*;
