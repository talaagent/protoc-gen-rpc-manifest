#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;
// reqwest and miette are std-only.
#[cfg(any(feature = "connect", feature = "miette", test))]
extern crate std;

#[cfg(feature = "connect")]
pub mod connect;
mod lookup;
mod manifest;

pub use lookup::{LookupError, Manifest, find_method};
pub use manifest::{Idempotency, Method, Service, Streaming};
