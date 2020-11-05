//! # `vapix`
//!
//! Client for [AXIS Communications](https://www.axis.com/en-us) devices' VAPIX API. Bullet points:

#![forbid(unsafe_code)]
//#![forbid(missing_docs)]
#![forbid(unused_variables)]

mod client;
mod error;
mod transport;

/// Define a type T which is `impl From<String> for T`, `impl From<T> for String`, and associated
/// string-ish behaviors.
macro_rules! string_type {
    (
    $(#[$doc:meta])*
    $v:vis struct $t:ident
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
        #[repr(transparent)]
        #[serde(transparent)]
        $v struct $t(String);

        impl $t {
            /// Creates a new `$t` from a `string`.
            pub fn new<S: Into<String>>(string: S) -> Self {
               Self(string.into())
            }

            /// Unwraps the value.
            pub fn into_inner(self) -> String {
                self.0
            }

            /// Returns a `&str`.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $t {
            fn from(v: String) -> Self {
                $t(v)
            }
        }
        impl From<&str> for $t {
            fn from(v: &str) -> Self {
                $t(v.to_owned())
            }
        }
        impl From<$t> for String {
            fn from(v: $t) -> Self {
                v.0
            }
        }
        impl<'a> From<&'a $t> for &'a str {
            fn from(v: &'a $t) -> Self {
                &v.0
            }
        }
        impl AsRef<str> for $t {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

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
