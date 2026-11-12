//! Plugin options: the comma-separated `parameter` of the request
//! (`--rpc-manifest_opt=...` or `opt:` in `buf.gen.yaml`).

use crate::Error;

/// An output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// A `.rs` with `pub static SERVICES: &[rpc_manifest::Service]`.
    Rust,
    /// A `.ts` with `export const SERVICES`.
    Ts,
    /// A `.json` document.
    Json,
}

impl Lang {
    /// The output file extension.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Ts => "ts",
            Self::Json => "json",
        }
    }
}

/// Parsed plugin options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// `lang=rust|ts|json`, repeatable; default `rust`.
    pub langs: Vec<Lang>,
    /// `package=<prefix>`, repeatable: keep services whose package is the
    /// prefix or under it (`acme.user` keeps `acme.user.v1`). Default: all.
    pub packages: Vec<String>,
    /// `alias=<full service name>:<alias>`, repeatable.
    pub aliases: Vec<(String, String)>,
    /// `filename=<base>`: output file name without extension; default
    /// `rpc_manifest`. May contain `/`.
    pub filename: String,
    /// `crate=<path>`: the path the Rust output uses for the types; default
    /// `::rpc_manifest`.
    pub crate_path: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            langs: Vec::new(),
            packages: Vec::new(),
            aliases: Vec::new(),
            filename: "rpc_manifest".into(),
            crate_path: "::rpc_manifest".into(),
        }
    }
}

impl Options {
    /// Parses `key=value,key=value`.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownOption`] or [`Error::InvalidOption`].
    pub fn parse(parameter: &str) -> Result<Self, Error> {
        let mut o = Self::default();
        for item in parameter
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let (key, value) = item.split_once('=').unwrap_or((item, ""));
            let invalid = |option, message: &str| Error::InvalidOption {
                option,
                message: message.into(),
            };
            match key {
                "lang" => o.langs.push(match value {
                    "rust" | "rs" => Lang::Rust,
                    "ts" | "typescript" => Lang::Ts,
                    "json" => Lang::Json,
                    _ => return Err(invalid("lang", "expected rust, ts or json")),
                }),
                "package" => {
                    if value.is_empty() {
                        return Err(invalid("package", "empty package prefix"));
                    }
                    o.packages.push(value.trim_start_matches('.').into());
                }
                "alias" => {
                    let (service, alias) = value
                        .split_once(':')
                        .filter(|(s, a)| !s.is_empty() && !a.is_empty())
                        .ok_or_else(|| invalid("alias", "expected <full.ServiceName>:<alias>"))?;
                    o.aliases
                        .push((service.trim_start_matches('.').into(), alias.into()));
                }
                "filename" => {
                    if value.is_empty() {
                        return Err(invalid("filename", "empty file name"));
                    }
                    value.clone_into(&mut o.filename);
                }
                "crate" => {
                    if value.is_empty() {
                        return Err(invalid("crate", "empty crate path"));
                    }
                    value.clone_into(&mut o.crate_path);
                }
                _ => return Err(Error::UnknownOption(key.into())),
            }
        }
        if o.langs.is_empty() {
            o.langs.push(Lang::Rust);
        }
        o.langs.dedup();
        Ok(o)
    }

    /// Whether `package` passes the `package=` filters.
    pub fn keeps(&self, package: &str) -> bool {
        self.packages.is_empty()
            || self.packages.iter().any(|p| {
                package
                    .strip_prefix(p.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_everything() {
        let o = Options::parse(
            "lang=rust, lang=ts,package=acme.user,alias=acme.user.v1.UserService:u,filename=gen/m,crate=crate::rpc",
        )
        .unwrap();
        assert_eq!(o.langs, [Lang::Rust, Lang::Ts]);
        assert_eq!(o.packages, ["acme.user"]);
        assert_eq!(o.aliases, [("acme.user.v1.UserService".into(), "u".into())]);
        assert_eq!(o.filename, "gen/m");
        assert_eq!(o.crate_path, "crate::rpc");
        assert_eq!(Options::parse("").unwrap().langs, [Lang::Rust]);
    }

    #[test]
    fn rejects_bad_options() {
        let code = |p| Options::parse(p).unwrap_err().code();
        assert_eq!(code("lang=go"), "invalid_option");
        assert_eq!(code("alias=Foo"), "invalid_option");
        assert_eq!(code("frobnicate"), "unknown_option");
    }

    #[test]
    fn package_prefix_is_segment_aware() {
        let o = Options::parse("package=acme.user").unwrap();
        assert!(o.keeps("acme.user"));
        assert!(o.keeps("acme.user.v1"));
        assert!(!o.keeps("acme.username.v1"));
        assert!(!o.keeps("acme"));
        assert!(Options::parse("").unwrap().keeps("anything"));
    }
}
