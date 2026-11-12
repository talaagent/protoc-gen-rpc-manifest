#![doc = include_str!("../README.md")]

extern crate alloc;

#[cfg(feature = "connect")]
pub mod connect;
mod lookup;
mod manifest;

pub use lookup::{LookupError, Manifest, find_method};
pub use manifest::{Idempotency, Method, Service, Streaming};
