//! protoc/buf plugin that emits a manifest of every RPC service and method
//! in the files selected for generation. See the workspace README for the
//! options and output formats.
//!
//! [`respond`] is the whole plugin; `main` only does the stdin/stdout
//! plumbing, so tests drive [`respond`] directly.

mod model;
mod options;
mod render;

use prost_types::compiler::code_generator_response::{Feature, File};
use prost_types::compiler::{CodeGeneratorRequest, CodeGeneratorResponse};

pub use model::{Method, Service};
pub use options::{Lang, Options};

/// Runs the plugin: parses the options, collects services and renders one
/// file per requested language. Errors land in the response's `error`.
pub fn respond(request: &CodeGeneratorRequest) -> CodeGeneratorResponse {
    let mut response = CodeGeneratorResponse {
        supported_features: Some(Feature::Proto3Optional as u64),
        ..CodeGeneratorResponse::default()
    };
    match generate(request) {
        Ok(files) => response.file = files,
        Err(e) => response.error = Some(e.to_string()),
    }
    response
}

/// [`respond`] with a typed error.
///
/// # Errors
///
/// Bad options, or aliases that don't resolve or collide.
pub fn generate(request: &CodeGeneratorRequest) -> Result<Vec<File>, Error> {
    let options = Options::parse(request.parameter.as_deref().unwrap_or(""))?;
    let services = model::collect(request, &options)?;
    Ok(options
        .langs
        .iter()
        .map(|&lang| File {
            name: Some(format!("{}.{}", options.filename, lang.extension())),
            content: Some(render::render(lang, &services, &options)),
            ..File::default()
        })
        .collect())
}

/// A plugin error. Every variant has a stable [`code`](Error::code).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// An option this plugin doesn't know.
    #[error("unknown option `{0}` (expected lang, package, alias, filename or crate)")]
    UnknownOption(String),
    /// An option with a bad value.
    #[error("invalid value for option `{option}`: {message}")]
    InvalidOption {
        /// The option's key.
        option: &'static str,
        /// What's wrong.
        message: String,
    },
    /// An `alias=` names a service that isn't generated.
    #[error("alias for unknown service `{0}`")]
    AliasForUnknownService(String),
    /// Two services ended up with the same alias.
    #[error("services `{first}` and `{second}` both have alias `{alias}`; set one with alias=")]
    DuplicateAlias {
        /// The alias.
        alias: String,
        /// One service's full name.
        first: String,
        /// The other's.
        second: String,
    },
}

impl Error {
    /// Stable `snake_case` code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownOption(_) => "unknown_option",
            Self::InvalidOption { .. } => "invalid_option",
            Self::AliasForUnknownService(_) => "alias_for_unknown_service",
            Self::DuplicateAlias { .. } => "duplicate_alias",
        }
    }
}
