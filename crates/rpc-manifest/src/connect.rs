//! A generic Connect unary JSON caller.
//!
//! [`Client::call`] POSTs a JSON body to `<base>/<service>/<Method>` with
//! `Content-Type: application/json` and `Connect-Protocol-Version: 1`, the
//! [Connect unary protocol]. A non-200 reply is decoded into a
//! [`ConnectError`] (`code`, `message`, `details`); when the body isn't a
//! Connect error, the code is inferred from the HTTP status as the spec says.
//!
//! [Connect unary protocol]: https://connectrpc.com/docs/protocol#unary-request

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;
use core::time::Duration;

use http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use crate::{Method, Service};

/// A boxed error from a [`HeaderHook`].
pub type BoxError = Box<dyn core::error::Error + Send + Sync>;

/// Adds headers to every request, e.g. auth. Runs just before sending, so a
/// hook can read a token that rotates.
pub trait HeaderHook: Send + Sync {
    /// Adds or replaces headers.
    ///
    /// # Errors
    ///
    /// Any error aborts the call with [`CallError::Hook`].
    fn apply(&self, headers: &mut HeaderMap) -> Result<(), BoxError>;
}

impl<F> HeaderHook for F
where
    F: Fn(&mut HeaderMap) -> Result<(), BoxError> + Send + Sync,
{
    fn apply(&self, headers: &mut HeaderMap) -> Result<(), BoxError> {
        self(headers)
    }
}

/// A hook that sets `Authorization: Bearer <token>`.
///
/// # Errors
///
/// [`CallError::InvalidHeader`] if the token isn't a valid header value.
pub fn bearer(token: &str) -> Result<impl HeaderHook + use<>, CallError> {
    let mut value = HeaderValue::try_from(alloc::format!("Bearer {token}"))
        .map_err(|_| CallError::InvalidHeader(AUTHORIZATION.as_str().into()))?;
    value.set_sensitive(true);
    Ok(move |h: &mut HeaderMap| {
        h.insert(AUTHORIZATION, value.clone());
        Ok(())
    })
}

/// A hook that sets one static header, e.g. `x-api-key`. The value is
/// marked sensitive so it doesn't show up in debug output.
///
/// # Errors
///
/// [`CallError::InvalidHeader`] if the name or value is invalid.
pub fn header(name: &str, value: &str) -> Result<impl HeaderHook + use<>, CallError> {
    let invalid = || CallError::InvalidHeader(name.into());
    let name = HeaderName::try_from(name).map_err(|_| invalid())?;
    let mut value = HeaderValue::try_from(value).map_err(|_| invalid())?;
    value.set_sensitive(true);
    Ok(move |h: &mut HeaderMap| {
        h.insert(name.clone(), value.clone());
        Ok(())
    })
}

/// Calls unary methods of one Connect server. Cheap to clone.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    hooks: Vec<Arc<dyn HeaderHook>>,
    timeout: Option<Duration>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("hooks", &self.hooks.len())
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client for `base` (e.g. `http://localhost:8080` or
    /// `https://api.example.com/rpc`) on a default `reqwest::Client`.
    pub fn new(base: impl Into<String>) -> Self {
        Self::with_http(reqwest::Client::new(), base)
    }

    /// A client on your own `reqwest::Client`, e.g. one built with
    /// `ClientBuilder::unix_socket` or custom TLS roots.
    pub fn with_http(http: reqwest::Client, base: impl Into<String>) -> Self {
        let mut base = base.into();
        while base.ends_with('/') {
            base.pop();
        }
        Self {
            http,
            base,
            hooks: Vec::new(),
            timeout: None,
        }
    }

    /// Adds a [`HeaderHook`]; hooks run in the order added.
    #[must_use]
    pub fn hook(mut self, hook: impl HeaderHook + 'static) -> Self {
        self.hooks.push(Arc::new(hook));
        self
    }

    /// Sets a per-call timeout, enforced locally and sent to the server as
    /// `Connect-Timeout-Ms`.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// The base URL, without a trailing slash.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Calls `service`/`method` from a manifest, refusing streaming methods
    /// (Connect unary JSON can't carry them).
    ///
    /// # Errors
    ///
    /// [`CallError::StreamingUnsupported`], or anything [`Client::call`]
    /// returns.
    pub async fn call_method(
        &self,
        service: &Service,
        method: &Method,
        body: impl Into<reqwest::Body>,
    ) -> Result<bytes::Bytes, CallError> {
        if !method.streaming.is_unary() {
            return Err(CallError::StreamingUnsupported {
                method: alloc::format!("{}/{}", service.full_name, method.name),
                streaming: method.streaming.as_str(),
            });
        }
        self.call(service.full_name, method.name, body).await
    }

    /// POSTs a JSON `body` to `<base>/<service>/<method>` and returns the
    /// raw JSON response body. The body isn't checked here; the server
    /// rejects malformed JSON with `invalid_argument`.
    ///
    /// # Errors
    ///
    /// [`CallError::Connect`] for an error reply; transport, timeout and
    /// hook errors otherwise.
    pub async fn call(
        &self,
        service: &str,
        method: &str,
        body: impl Into<reqwest::Body>,
    ) -> Result<bytes::Bytes, CallError> {
        let url = alloc::format!("{}/{service}/{method}", self.base);
        let mut headers = HeaderMap::with_capacity(3 + self.hooks.len());
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("connect-protocol-version"),
            HeaderValue::from_static("1"),
        );
        let mut req = self.http.post(url);
        if let Some(t) = self.timeout {
            headers.insert(
                HeaderName::from_static("connect-timeout-ms"),
                HeaderValue::from(u64::try_from(t.as_millis()).unwrap_or(u64::MAX)),
            );
            req = req.timeout(t);
        }
        for hook in &self.hooks {
            hook.apply(&mut headers).map_err(CallError::Hook)?;
        }
        let resp = req
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(CallError::from_reqwest)?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(CallError::from_reqwest)?;
        if status.is_success() {
            Ok(bytes)
        } else {
            Err(CallError::Connect(ConnectError::from_response(
                status.as_u16(),
                &bytes,
            )))
        }
    }

    /// [`Client::call`] with typed request and response.
    ///
    /// # Errors
    ///
    /// As [`Client::call`], plus [`CallError::Encode`] /
    /// [`CallError::Decode`].
    pub async fn call_json<Req, Res>(
        &self,
        service: &str,
        method: &str,
        request: &Req,
    ) -> Result<Res, CallError>
    where
        Req: serde::Serialize + ?Sized,
        Res: serde::de::DeserializeOwned,
    {
        let body = serde_json::to_vec(request).map_err(CallError::Encode)?;
        let bytes = self.call(service, method, body).await?;
        serde_json::from_slice(&bytes).map_err(CallError::Decode)
    }
}

/// A failed call. Every variant has a stable [`code`](CallError::code).
#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "miette", derive(miette::Diagnostic))]
#[non_exhaustive]
pub enum CallError {
    /// The server replied with a Connect error.
    #[error(transparent)]
    #[cfg_attr(feature = "miette", diagnostic(transparent))]
    Connect(ConnectError),
    /// The local timeout expired.
    #[error("request timed out")]
    #[cfg_attr(feature = "miette", diagnostic(code(timeout)))]
    Timeout(#[source] reqwest::Error),
    /// Connecting, sending or reading failed.
    #[error("transport error: {}", Chain(.0))]
    #[cfg_attr(feature = "miette", diagnostic(code(transport)))]
    Transport(#[source] reqwest::Error),
    /// A [`HeaderHook`] failed.
    #[error("header hook failed: {0}")]
    #[cfg_attr(feature = "miette", diagnostic(code(header_hook)))]
    Hook(#[source] BoxError),
    /// A header name or value was invalid.
    #[error("invalid header `{0}`")]
    #[cfg_attr(feature = "miette", diagnostic(code(invalid_header)))]
    InvalidHeader(String),
    /// The method streams; only unary methods can be called.
    #[error("`{method}` is a {streaming}-streaming method; only unary methods can be called")]
    #[cfg_attr(feature = "miette", diagnostic(code(streaming_unsupported)))]
    StreamingUnsupported {
        /// `service/Method`.
        method: String,
        /// The streaming kind.
        streaming: &'static str,
    },
    /// The request didn't serialize to JSON.
    #[error("could not encode request: {0}")]
    #[cfg_attr(feature = "miette", diagnostic(code(encode_request)))]
    Encode(#[source] serde_json::Error),
    /// The response wasn't the expected JSON.
    #[error("could not decode response: {0}")]
    #[cfg_attr(feature = "miette", diagnostic(code(decode_response)))]
    Decode(#[source] serde_json::Error),
}

impl CallError {
    fn from_reqwest(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout(e)
        } else {
            Self::Transport(e)
        }
    }

    /// Stable `snake_case` code; for a server error, its Connect code
    /// (`not_found`, `unauthenticated`, …).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Connect(e) => e.code.as_str(),
            Self::Timeout(_) => "timeout",
            Self::Transport(_) => "transport",
            Self::Hook(_) => "header_hook",
            Self::InvalidHeader(_) => "invalid_header",
            Self::StreamingUnsupported { .. } => "streaming_unsupported",
            Self::Encode(_) => "encode_request",
            Self::Decode(_) => "decode_response",
        }
    }
}

/// An error and its sources, `: `-separated (reqwest's own `Display`
/// leaves out the cause, e.g. "connection refused").
struct Chain<'a>(&'a (dyn core::error::Error + 'static));

impl fmt::Display for Chain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;
        let mut source = self.0.source();
        while let Some(e) = source {
            write!(f, ": {e}")?;
            source = e.source();
        }
        Ok(())
    }
}

/// A Connect error reply.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ConnectError {
    /// The Connect code.
    pub code: Code,
    /// The human message (may be empty).
    pub message: String,
    /// Typed error details.
    pub details: Vec<ErrorDetail>,
    /// The HTTP status of the reply.
    pub http_status: u16,
}

#[cfg(feature = "miette")]
impl miette::Diagnostic for ConnectError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(self.code.as_str()))
    }
}

/// One entry of a Connect error's `details`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct ErrorDetail {
    /// Fully qualified message type, e.g. `google.rpc.BadRequest`.
    #[serde(rename = "type")]
    pub type_name: String,
    /// The message's protobuf binary encoding, base64 (as sent).
    pub value: String,
    /// The server's optional JSON rendering of the message.
    #[serde(default)]
    pub debug: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct WireError {
    code: Option<String>,
    #[serde(default)]
    message: String,
    #[serde(default)]
    details: Vec<ErrorDetail>,
}

impl ConnectError {
    /// Decodes an error reply, falling back to the HTTP status mapping when
    /// the body isn't a Connect error.
    pub fn from_response(http_status: u16, body: &[u8]) -> Self {
        let wire = serde_json::from_slice::<WireError>(body).ok();
        let code = wire
            .as_ref()
            .and_then(|w| w.code.as_deref())
            .and_then(Code::from_name)
            .unwrap_or_else(|| Code::from_http_status(http_status));
        match wire {
            Some(w) => Self {
                code,
                message: w.message,
                details: w.details,
                http_status,
            },
            None => Self {
                code,
                message: String::from_utf8_lossy(body).trim().into(),
                details: Vec::new(),
                http_status,
            },
        }
    }
}

macro_rules! codes {
    ($($(#[$doc:meta])* $variant:ident = $name:literal,)*) => {
        /// A Connect (and gRPC) status code.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Code {
            $($(#[$doc])* $variant,)*
        }

        impl Code {
            /// All codes.
            pub const ALL: &[Code] = &[$(Self::$variant),*];

            /// The protocol's `snake_case` name.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            /// Parses a protocol name.
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

codes! {
    /// The call was cancelled.
    Canceled = "canceled",
    /// Unknown error.
    Unknown = "unknown",
    /// The request was invalid.
    InvalidArgument = "invalid_argument",
    /// The deadline passed.
    DeadlineExceeded = "deadline_exceeded",
    /// Not found.
    NotFound = "not_found",
    /// Already exists.
    AlreadyExists = "already_exists",
    /// The caller may not do this.
    PermissionDenied = "permission_denied",
    /// Out of quota or capacity.
    ResourceExhausted = "resource_exhausted",
    /// The system isn't in a state for this.
    FailedPrecondition = "failed_precondition",
    /// Aborted, e.g. by a conflict.
    Aborted = "aborted",
    /// Out of range.
    OutOfRange = "out_of_range",
    /// Not implemented, or no such method.
    Unimplemented = "unimplemented",
    /// Internal error.
    Internal = "internal",
    /// Temporarily unavailable.
    Unavailable = "unavailable",
    /// Unrecoverable data loss.
    DataLoss = "data_loss",
    /// No valid credentials.
    Unauthenticated = "unauthenticated",
}

impl Code {
    /// The code the Connect spec infers from an HTTP status when the body
    /// isn't a Connect error.
    pub const fn from_http_status(status: u16) -> Self {
        match status {
            400 => Self::Internal,
            401 => Self::Unauthenticated,
            403 => Self::PermissionDenied,
            404 => Self::Unimplemented,
            429 | 502..=504 => Self::Unavailable,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_connect_error() {
        let body = br#"{"code":"not_found","message":"no user 7","details":[{"type":"acme.Thing","value":"CgE3","debug":{"id":"7"}}]}"#;
        let e = ConnectError::from_response(404, body);
        assert_eq!(e.code, Code::NotFound);
        assert_eq!(e.message, "no user 7");
        assert_eq!(e.details[0].type_name, "acme.Thing");
        assert_eq!(e.details[0].value, "CgE3");
        assert_eq!(e.to_string(), "not_found: no user 7");
    }

    #[test]
    fn falls_back_to_http_status() {
        let e = ConnectError::from_response(503, b"upstream down\n");
        assert_eq!(e.code, Code::Unavailable);
        assert_eq!(e.message, "upstream down");
        let e = ConnectError::from_response(418, br#"{"code":"bogus"}"#);
        assert_eq!(e.code, Code::Unknown);
    }

    #[test]
    fn code_names_round_trip() {
        for &c in Code::ALL {
            assert_eq!(Code::from_name(c.as_str()), Some(c));
        }
    }
}
