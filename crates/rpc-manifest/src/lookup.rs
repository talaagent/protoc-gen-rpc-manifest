//! Name lookups with forgiving matching and did-you-mean suggestions.
//!
//! A query matches exactly first (full name, alias, or short name for
//! services; method name for methods), then loosely: case-insensitive with
//! `-`, `_` and `.` ignored, so `list-users`, `list_users` and `listusers`
//! all find `ListUsers`.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::{Method, Service};

/// How many suggestions an error carries at most.
const MAX_SUGGESTIONS: usize = 3;

/// A set of services, usually the generated `SERVICES` static.
#[derive(Debug, Clone, Copy)]
pub struct Manifest {
    services: &'static [Service],
}

impl Manifest {
    /// Wraps a generated service list.
    pub const fn new(services: &'static [Service]) -> Self {
        Self { services }
    }

    /// All services, sorted by full name.
    pub const fn services(&self) -> &'static [Service] {
        self.services
    }

    /// Finds a service by full name, alias or short name.
    ///
    /// # Errors
    ///
    /// [`LookupError::UnknownService`] with suggestions when nothing matches,
    /// [`LookupError::AmbiguousService`] when a loose match hits several.
    pub fn service(&self, query: &str) -> Result<&'static Service, LookupError> {
        let exact = self
            .services
            .iter()
            .find(|s| s.full_name == query || s.alias == query);
        if let Some(s) = exact {
            return Ok(s);
        }
        let keys = |s: &'static Service| [s.full_name, s.alias, s.name];
        let found = loose_matches(self.services, query, keys);
        match found.as_slice() {
            [one] => Ok(one),
            [] => Err(LookupError::UnknownService {
                query: query.into(),
                suggestions: suggest(query, self.services.iter().map(|s| s.alias)),
            }),
            many => Err(LookupError::AmbiguousService {
                query: query.into(),
                candidates: many.iter().map(|s| s.full_name).collect(),
            }),
        }
    }

    /// Finds `service` then its `method`; see [`Manifest::service`] and
    /// [`find_method`].
    ///
    /// # Errors
    ///
    /// Any [`LookupError`].
    pub fn method(
        &self,
        service: &str,
        method: &str,
    ) -> Result<(&'static Service, &'static Method), LookupError> {
        let s = self.service(service)?;
        Ok((s, find_method(s, method)?))
    }
}

/// Finds a method of `service` by name.
///
/// # Errors
///
/// [`LookupError::UnknownMethod`] with suggestions when nothing matches.
pub fn find_method(service: &'static Service, query: &str) -> Result<&'static Method, LookupError> {
    if let Some(m) = service.methods.iter().find(|m| m.name == query) {
        return Ok(m);
    }
    // Method names are unique within a service, and so are their loose
    // forms in any sane proto, so the first loose hit is the answer.
    loose_matches(service.methods, query, |m| [m.name])
        .first()
        .copied()
        .ok_or_else(|| LookupError::UnknownMethod {
            service: service.full_name,
            query: query.into(),
            suggestions: suggest(query, service.methods.iter().map(|m| m.name)),
        })
}

/// A failed lookup. Every variant has a stable [`code`](LookupError::code).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "miette", derive(miette::Diagnostic))]
pub enum LookupError {
    /// No service matched.
    #[error("unknown service `{query}`{}", DidYouMean(suggestions))]
    #[cfg_attr(feature = "miette", diagnostic(code(unknown_service)))]
    UnknownService {
        /// What was asked for.
        query: String,
        /// Close aliases, best first.
        suggestions: Vec<&'static str>,
    },
    /// A loose match hit more than one service.
    #[error("`{query}` matches several services: {}", candidates.join(", "))]
    #[cfg_attr(feature = "miette", diagnostic(code(ambiguous_service)))]
    AmbiguousService {
        /// What was asked for.
        query: String,
        /// Full names of every match.
        candidates: Vec<&'static str>,
    },
    /// No method of the service matched.
    #[error("unknown method `{query}` on `{service}`{}", DidYouMean(suggestions))]
    #[cfg_attr(feature = "miette", diagnostic(code(unknown_method)))]
    UnknownMethod {
        /// The service's full name.
        service: &'static str,
        /// What was asked for.
        query: String,
        /// Close method names, best first.
        suggestions: Vec<&'static str>,
    },
}

impl LookupError {
    /// Stable `snake_case` error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownService { .. } => "unknown_service",
            Self::AmbiguousService { .. } => "ambiguous_service",
            Self::UnknownMethod { .. } => "unknown_method",
        }
    }

    /// The did-you-mean candidates, best first (empty if none).
    pub fn suggestions(&self) -> &[&'static str] {
        match self {
            Self::UnknownService { suggestions, .. } | Self::UnknownMethod { suggestions, .. } => {
                suggestions
            }
            Self::AmbiguousService { candidates, .. } => candidates,
        }
    }
}

struct DidYouMean<'a>(&'a [&'static str]);

impl fmt::Display for DidYouMean<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            [] => Ok(()),
            [one] => write!(f, "; did you mean `{one}`?"),
            many => {
                f.write_str("; did you mean one of ")?;
                for (i, s) in many.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "`{s}`")?;
                }
                f.write_str("?")
            }
        }
    }
}

/// Lowercased ASCII alphanumerics only: the loose form of a name.
fn normalize(s: &str) -> impl Iterator<Item = u8> + '_ {
    s.bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(|b| b.to_ascii_lowercase())
}

fn loose_eq(a: &str, b: &str) -> bool {
    normalize(a).eq(normalize(b))
}

fn loose_matches<T, const N: usize>(
    items: &'static [T],
    query: &str,
    keys: impl Fn(&'static T) -> [&'static str; N],
) -> Vec<&'static T> {
    items
        .iter()
        .filter(|item| keys(item).iter().any(|k| loose_eq(k, query)))
        .collect()
}

/// Candidates within a small edit distance of `query` (loose forms), or
/// that contain it, best first.
fn suggest(query: &str, candidates: impl Iterator<Item = &'static str>) -> Vec<&'static str> {
    let q: Vec<u8> = normalize(query).collect();
    if q.is_empty() {
        return Vec::new();
    }
    let limit = (q.len() / 3).max(2);
    let mut scored: Vec<(usize, &'static str)> = candidates
        .filter_map(|c| {
            let k: Vec<u8> = normalize(c).collect();
            let d = levenshtein(&q, &k);
            let contains = k.windows(q.len()).any(|w| w == q.as_slice());
            (d <= limit || contains).then_some((if contains { 0 } else { d }, c))
        })
        .collect();
    scored.sort_unstable();
    scored.dedup_by_key(|(_, c)| *c);
    scored
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, c)| c)
        .collect()
}

fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = if ca == cb {
                prev
            } else {
                1 + prev.min(cur).min(row[j])
            };
            prev = cur;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Idempotency, Streaming};
    use alloc::string::ToString;

    const fn method(name: &'static str) -> Method {
        Method {
            name,
            input_type: "t.v1.Req",
            output_type: "t.v1.Res",
            idempotency: Idempotency::Unknown,
            streaming: Streaming::Unary,
            comments: "",
        }
    }

    static SERVICES: &[Service] = &[
        Service {
            full_name: "t.v1.UserService",
            package: "t.v1",
            name: "UserService",
            alias: "user",
            comments: "Users.\nMore.",
            methods: &[method("GetUser"), method("ListUsers")],
        },
        Service {
            full_name: "t.v1.UserAdminService",
            package: "t.v1",
            name: "UserAdminService",
            alias: "user-admin",
            comments: "",
            methods: &[method("BanUser")],
        },
        Service {
            full_name: "t.v2.UserAdminService",
            package: "t.v2",
            name: "UserAdminService",
            alias: "user-admin-v2",
            comments: "",
            methods: &[],
        },
    ];
    const M: Manifest = Manifest::new(SERVICES);

    #[test]
    fn finds_services_exactly_and_loosely() {
        assert_eq!(M.service("user").unwrap().full_name, "t.v1.UserService");
        assert_eq!(M.service("t.v1.UserService").unwrap().alias, "user");
        assert_eq!(M.service("User").unwrap().alias, "user");
        assert_eq!(M.service("USER_ADMIN").unwrap().alias, "user-admin");
        assert_eq!(M.service("userservice").unwrap().alias, "user");
    }

    #[test]
    fn ambiguous_short_name() {
        let err = M.service("useradminservice").unwrap_err();
        assert_eq!(err.code(), "ambiguous_service");
        assert_eq!(
            err.suggestions(),
            ["t.v1.UserAdminService", "t.v2.UserAdminService"]
        );
    }

    #[test]
    fn unknown_service_suggests() {
        let err = M.service("usr").unwrap_err();
        assert_eq!(err.code(), "unknown_service");
        assert_eq!(err.suggestions()[0], "user");
        assert_eq!(
            err.to_string(),
            "unknown service `usr`; did you mean `user`?"
        );
        assert_eq!(
            M.service("zzzzzzzz").unwrap_err().suggestions(),
            [] as [&str; 0]
        );
    }

    #[test]
    fn methods() {
        let (s, m) = M.method("user", "list-users").unwrap();
        assert_eq!((s.alias, m.name), ("user", "ListUsers"));
        assert_eq!(M.method("user", "getuser").unwrap().1.name, "GetUser");
        let err = M.method("user", "GetUsr").unwrap_err();
        assert_eq!(err.code(), "unknown_method");
        assert_eq!(
            err.to_string(),
            "unknown method `GetUsr` on `t.v1.UserService`; did you mean `GetUser`?"
        );
        assert_eq!(SERVICES[0].summary(), "Users.");
    }

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein(b"kitten", b"sitting"), 3);
        assert_eq!(levenshtein(b"", b"abc"), 3);
        assert_eq!(levenshtein(b"same", b"same"), 0);
    }
}
