//! # `vapix`
//!
//! Client for [AXIS Communications](https://www.axis.com/en-us) devices' VAPIX API. Bullet points:

#![forbid(unsafe_code)]
//#![forbid(missing_docs)]
#![forbid(unused_variables)]

mod client;
mod error;
mod transport;

pub mod v3;
pub mod v4;

pub use client::Client;
pub(crate) use error::ResultExt;
pub use error::{Error, Result};
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
