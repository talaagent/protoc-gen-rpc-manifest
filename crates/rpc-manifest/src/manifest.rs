//! The manifest data model, as emitted by `protoc-gen-rpc-manifest lang=rust`.

/// One RPC service from the selected `.proto` files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Service {
    /// Fully qualified name, e.g. `acme.user.v1.UserService`.
    pub full_name: &'static str,
    /// Proto package, e.g. `acme.user.v1` (empty when the file has none).
    pub package: &'static str,
    /// Unqualified name, e.g. `UserService`.
    pub name: &'static str,
    /// Short CLI alias, e.g. `user`.
    pub alias: &'static str,
    /// Leading comment on the service declaration, trimmed.
    pub comments: &'static str,
    /// Methods in declaration order.
    pub methods: &'static [Method],
}

/// One RPC method of a [`Service`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Method {
    /// Method name as declared, e.g. `GetUser`.
    pub name: &'static str,
    /// Fully qualified request message, e.g. `acme.user.v1.GetUserRequest`.
    pub input_type: &'static str,
    /// Fully qualified response message.
    pub output_type: &'static str,
    /// The `idempotency_level` method option.
    pub idempotency: Idempotency,
    /// Which sides stream.
    pub streaming: Streaming,
    /// Leading comment on the method declaration, trimmed.
    pub comments: &'static str,
}

/// The `google.protobuf.MethodOptions.idempotency_level` of a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Idempotency {
    /// Not declared.
    #[default]
    Unknown,
    /// `NO_SIDE_EFFECTS`: safe to call any number of times, reads only.
    NoSideEffects,
    /// `IDEMPOTENT`: may have side effects, but repeating is harmless.
    Idempotent,
}

impl Idempotency {
    /// Stable `snake_case` name, matching the JSON and TS output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::NoSideEffects => "no_side_effects",
            Self::Idempotent => "idempotent",
        }
    }
}

/// Which sides of a method stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Streaming {
    /// One request, one response.
    #[default]
    Unary,
    /// The client streams requests.
    Client,
    /// The server streams responses.
    Server,
    /// Both sides stream.
    Bidi,
}

impl Streaming {
    /// Stable `snake_case` name, matching the JSON and TS output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unary => "unary",
            Self::Client => "client",
            Self::Server => "server",
            Self::Bidi => "bidi",
        }
    }

    /// Whether this is a plain unary method (callable with Connect unary JSON).
    pub const fn is_unary(self) -> bool {
        matches!(self, Self::Unary)
    }
}

impl Service {
    /// The first line of [`Service::comments`], for one-line listings.
    pub fn summary(&self) -> &'static str {
        first_line(self.comments)
    }
}

impl Method {
    /// The first line of [`Method::comments`], for one-line listings.
    pub fn summary(&self) -> &'static str {
        first_line(self.comments)
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}
