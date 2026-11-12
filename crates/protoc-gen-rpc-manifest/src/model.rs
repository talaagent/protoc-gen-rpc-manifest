//! Collects services from the request's descriptors.

use std::collections::{HashMap, HashSet};

use prost_types::compiler::CodeGeneratorRequest;
use prost_types::method_options::IdempotencyLevel;
use prost_types::{FileDescriptorProto, SourceCodeInfo};

use crate::{Error, Options};

/// `FileDescriptorProto.service` field number.
const FILE_SERVICE: i32 = 6;
/// `ServiceDescriptorProto.method` field number.
const SERVICE_METHOD: i32 = 2;

/// A service, ready to render.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Service {
    /// `acme.user.v1.UserService`.
    pub full_name: String,
    /// `acme.user.v1`.
    pub package: String,
    /// `UserService`.
    pub name: String,
    /// `user`.
    pub alias: String,
    /// Leading comment, cleaned.
    pub comments: String,
    /// Methods in declaration order.
    pub methods: Vec<Method>,
}

/// A method, ready to render.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Method {
    /// `GetUser`.
    pub name: String,
    /// Fully qualified, no leading dot.
    pub input_type: String,
    /// Fully qualified, no leading dot.
    pub output_type: String,
    /// `unknown`, `no_side_effects` or `idempotent`.
    pub idempotency: &'static str,
    /// `unary`, `client`, `server` or `bidi`.
    pub streaming: &'static str,
    /// Leading comment, cleaned.
    pub comments: String,
}

/// Services in the files to generate that pass the package filter, sorted
/// by full name, with aliases assigned.
pub fn collect(request: &CodeGeneratorRequest, options: &Options) -> Result<Vec<Service>, Error> {
    // Descriptors for imports arrive too; only the selected files count.
    let selected: HashSet<&str> = request
        .file_to_generate
        .iter()
        .map(String::as_str)
        .collect();
    let mut services: Vec<Service> = request
        .proto_file
        .iter()
        .filter(|f| selected.contains(f.name()) && options.keeps(f.package()))
        .flat_map(file_services)
        .collect();
    services.sort_unstable_by(|a, b| a.full_name.cmp(&b.full_name));
    assign_aliases(&mut services, &options.aliases)?;
    Ok(services)
}

fn file_services(file: &FileDescriptorProto) -> impl Iterator<Item = Service> + '_ {
    let comments = Comments::new(file.source_code_info.as_ref());
    let package = file.package();
    file.service.iter().enumerate().map(move |(si, svc)| {
        let si = i32::try_from(si).unwrap_or(i32::MAX);
        let methods = svc
            .method
            .iter()
            .enumerate()
            .map(|(mi, m)| {
                let mi = i32::try_from(mi).unwrap_or(i32::MAX);
                Method {
                    name: m.name().into(),
                    input_type: m.input_type().trim_start_matches('.').into(),
                    output_type: m.output_type().trim_start_matches('.').into(),
                    idempotency: match m
                        .options
                        .as_ref()
                        .map(prost_types::MethodOptions::idempotency_level)
                    {
                        Some(IdempotencyLevel::NoSideEffects) => "no_side_effects",
                        Some(IdempotencyLevel::Idempotent) => "idempotent",
                        _ => "unknown",
                    },
                    streaming: match (m.client_streaming(), m.server_streaming()) {
                        (false, false) => "unary",
                        (true, false) => "client",
                        (false, true) => "server",
                        (true, true) => "bidi",
                    },
                    comments: comments.leading(&[FILE_SERVICE, si, SERVICE_METHOD, mi]),
                }
            })
            .collect();
        let name = svc.name();
        Service {
            full_name: if package.is_empty() {
                name.into()
            } else {
                format!("{package}.{name}")
            },
            package: package.into(),
            name: name.into(),
            alias: String::new(),
            comments: comments.leading(&[FILE_SERVICE, si]),
            methods,
        }
    })
}

/// Leading comments of a file's declarations, by source path.
struct Comments<'a>(HashMap<&'a [i32], &'a str>);

impl<'a> Comments<'a> {
    fn new(info: Option<&'a SourceCodeInfo>) -> Self {
        Self(
            info.into_iter()
                .flat_map(|i| &i.location)
                .filter_map(|l| Some((l.path.as_slice(), l.leading_comments.as_deref()?)))
                .collect(),
        )
    }

    fn leading(&self, path: &[i32]) -> String {
        self.0
            .get(path)
            .map_or_else(String::new, |c| clean_comment(c))
    }
}

/// Drops the one space protoc leaves after `//`, trailing whitespace and
/// blank lines at either end.
fn clean_comment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line.strip_prefix(' ').unwrap_or(line).trim_end());
    }
    out.trim_matches('\n').into()
}

/// `UserAdminService` → `user-admin`.
fn default_alias(name: &str) -> String {
    let base = name
        .strip_suffix("Service")
        .filter(|b| !b.is_empty())
        .unwrap_or(name);
    let chars: Vec<char> = base.chars().collect();
    let mut out = String::with_capacity(base.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev = i.checked_sub(1).map(|p| chars[p]);
            let next = chars.get(i + 1);
            // A word starts at `aB`, `2B`, and at the last capital of an
            // acronym followed by lowercase (`HTTPProxy` → `http-proxy`).
            let boundary = prev.is_some_and(|p| {
                p.is_ascii_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_ascii_uppercase() && next.is_some_and(char::is_ascii_lowercase))
            });
            if boundary {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(if c == '_' { '-' } else { c });
        }
    }
    out
}

/// Explicit aliases win; default aliases that collide get the last package
/// segment appended (`user-v1`, `user-v2`); anything still colliding is an
/// error.
fn assign_aliases(services: &mut [Service], explicit: &[(String, String)]) -> Result<(), Error> {
    for (full, _) in explicit {
        if !services.iter().any(|s| &s.full_name == full) {
            return Err(Error::AliasForUnknownService(full.clone()));
        }
    }
    let explicit_for = |s: &Service| {
        explicit
            .iter()
            .rev()
            .find(|(full, _)| *full == s.full_name)
            .map(|(_, a)| a.clone())
    };
    for s in services.iter_mut() {
        s.alias = explicit_for(s).unwrap_or_else(|| default_alias(&s.name));
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for s in services.iter() {
        *counts.entry(s.alias.clone()).or_default() += 1;
    }
    for s in services.iter_mut() {
        if counts[&s.alias] > 1 && explicit_for(s).is_none() {
            if let Some(last) = s.package.rsplit('.').next().filter(|l| !l.is_empty()) {
                s.alias = format!("{}-{}", s.alias, default_alias(last));
            }
        }
    }
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for s in services.iter() {
        if let Some(first) = seen.insert(&s.alias, &s.full_name) {
            return Err(Error::DuplicateAlias {
                alias: s.alias.clone(),
                first: first.into(),
                second: s.full_name.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_aliases() {
        assert_eq!(default_alias("UserService"), "user");
        assert_eq!(default_alias("UserAdminService"), "user-admin");
        assert_eq!(default_alias("Widget"), "widget");
        assert_eq!(default_alias("Service"), "service");
        assert_eq!(default_alias("HTTPProxyService"), "http-proxy");
        assert_eq!(default_alias("V2Thing"), "v2-thing");
        assert_eq!(default_alias("snake_case"), "snake-case");
    }

    #[test]
    fn cleans_comments() {
        assert_eq!(
            clean_comment(" Line one.\n  indented\n\n"),
            "Line one.\n indented"
        );
        assert_eq!(clean_comment("\n x \n"), "x");
    }

    fn svc(full: &str) -> Service {
        let (package, name) = full.rsplit_once('.').unwrap_or(("", full));
        Service {
            full_name: full.into(),
            package: package.into(),
            name: name.into(),
            alias: String::new(),
            comments: String::new(),
            methods: Vec::new(),
        }
    }

    #[test]
    fn colliding_aliases_get_package_suffix() {
        let mut s = vec![
            svc("a.v1.UserService"),
            svc("a.v2.UserService"),
            svc("a.v1.Other"),
        ];
        assign_aliases(&mut s, &[]).unwrap();
        let aliases: Vec<_> = s.iter().map(|s| s.alias.as_str()).collect();
        assert_eq!(aliases, ["user-v1", "user-v2", "other"]);

        let mut s = vec![svc("a.v1.UserService"), svc("a.v2.UserService")];
        assign_aliases(&mut s, &[("a.v2.UserService".into(), "u2".into())]).unwrap();
        let aliases: Vec<_> = s.iter().map(|s| s.alias.as_str()).collect();
        assert_eq!(aliases, ["user", "u2"]);
    }

    #[test]
    fn alias_errors() {
        let mut s = vec![svc("a.v1.UserService"), svc("a.v1.Other")];
        let err = assign_aliases(&mut s, &[("a.v1.Nope".into(), "n".into())]).unwrap_err();
        assert_eq!(err.code(), "alias_for_unknown_service");
        // An explicit alias wins over a default one, which moves aside...
        assign_aliases(&mut s, &[("a.v1.Other".into(), "user".into())]).unwrap();
        assert_eq!(
            (s[0].alias.as_str(), s[1].alias.as_str()),
            ("user-v1", "user")
        );
        // ...but two explicit ones can't share.
        let both = [
            ("a.v1.Other".into(), "x".into()),
            ("a.v1.UserService".into(), "x".into()),
        ];
        let err = assign_aliases(&mut s, &both).unwrap_err();
        assert_eq!(err.code(), "duplicate_alias");
    }
}
