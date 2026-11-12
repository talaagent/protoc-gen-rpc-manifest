#![doc = include_str!("../README.md")]

mod args;
mod output;

use std::io::{self, Read as _, Write};
use std::path::PathBuf;

use rpc_manifest::connect::{CallError, Client};
use rpc_manifest::{LookupError, Manifest, Method, Service, find_method};

pub use args::RpcArgs;
#[cfg(feature = "bpaf")]
pub use args::{rpc_args, rpc_command};

/// What an [`RpcArgs`] asks for, with names resolved and the body read.
#[derive(Debug, Clone)]
pub enum Action {
    /// No service given: list the services.
    ListServices(Manifest),
    /// A service but no method: list its methods.
    ListMethods(&'static Service),
    /// A method but no body: show its docs.
    Describe(&'static Service, &'static Method),
    /// Everything given: call it with this JSON body (already validated).
    Call(&'static Service, &'static Method, Vec<u8>),
}

impl RpcArgs {
    /// Resolves names against `manifest` and reads the body (`@file`, `-`
    /// for stdin, or literal JSON).
    ///
    /// # Errors
    ///
    /// [`Error::Lookup`] with suggestions for a bad name; body read or JSON
    /// errors.
    pub fn resolve(&self, manifest: Manifest) -> Result<Action, Error> {
        let Some(service) = &self.service else {
            return Ok(Action::ListServices(manifest));
        };
        let service = manifest.service(service)?;
        let Some(method) = &self.method else {
            return Ok(Action::ListMethods(service));
        };
        let method = find_method(service, method)?;
        let Some(body) = &self.body else {
            return Ok(Action::Describe(service, method));
        };
        let body = read_body(body)?;
        serde_json::from_slice::<serde::de::IgnoredAny>(&body).map_err(Error::InvalidJson)?;
        Ok(Action::Call(service, method, body))
    }
}

fn read_body(arg: &str) -> Result<Vec<u8>, Error> {
    if arg == "-" {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .map_err(|source| Error::ReadBody {
                path: "-".into(),
                source,
            })?;
        Ok(buf)
    } else if let Some(path) = arg.strip_prefix('@') {
        std::fs::read(path).map_err(|source| Error::ReadBody {
            path: path.into(),
            source,
        })
    } else {
        Ok(arg.as_bytes().to_vec())
    }
}

impl Action {
    /// Prints a listing or docs, or performs the call and pretty-prints
    /// the JSON response, to `out`. `prog` is how the help text refers to
    /// the subcommand, e.g. `myadmin rpc`.
    ///
    /// # Errors
    ///
    /// [`Error::Call`] for a failed call; [`Error::Io`] writing `out`.
    pub async fn run(self, client: &Client, prog: &str, out: &mut impl Write) -> Result<(), Error> {
        match self {
            Self::ListServices(m) => output::services(m.services(), prog, out)?,
            Self::ListMethods(s) => output::methods(s, prog, out)?,
            Self::Describe(s, m) => output::describe(s, m, prog, out)?,
            Self::Call(s, m, body) => {
                let res = client.call_method(s, m, body).await?;
                output::response(&res, out)?;
            }
        }
        Ok(())
    }

    /// Whether running this action talks to the server, so callers can
    /// skip building a [`Client`] (and any auth) for listings.
    pub const fn needs_client(&self) -> bool {
        matches!(self, Self::Call(..))
    }
}

/// [`RpcArgs::resolve`] then [`Action::run`].
///
/// # Errors
///
/// Any [`Error`].
pub async fn run(
    manifest: Manifest,
    client: &Client,
    args: &RpcArgs,
    prog: &str,
    out: &mut impl Write,
) -> Result<(), Error> {
    args.resolve(manifest)?.run(client, prog, out).await
}

/// An `rpc` subcommand error. Every variant has a stable
/// [`code`](Error::code).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A service or method name didn't resolve.
    #[error(transparent)]
    Lookup(#[from] LookupError),
    /// The call failed.
    #[error(transparent)]
    Call(#[from] CallError),
    /// The body file or stdin couldn't be read.
    #[error("reading request body from {}: {source}", path.display())]
    ReadBody {
        /// The file, or `-` for stdin.
        path: PathBuf,
        /// The I/O error.
        source: io::Error,
    },
    /// The request body isn't JSON.
    #[error("request body is not valid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),
    /// Writing the output failed.
    #[error("writing output: {0}")]
    Io(#[from] io::Error),
}

impl Error {
    /// Stable `snake_case` code; lookup and call errors pass theirs through.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Lookup(e) => e.code(),
            Self::Call(e) => e.code(),
            Self::ReadBody { .. } => "read_body",
            Self::InvalidJson(_) => "invalid_json",
            Self::Io(_) => "io",
        }
    }
}

#[cfg(feature = "miette")]
impl miette::Diagnostic for Error {
    fn code<'a>(&'a self) -> Option<Box<dyn core::fmt::Display + 'a>> {
        Some(Box::new(Error::code(self)))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn core::fmt::Display + 'a>> {
        match self {
            Self::Lookup(e) => miette::Diagnostic::help(e),
            Self::Call(e) => miette::Diagnostic::help(e),
            _ => None,
        }
    }
}
