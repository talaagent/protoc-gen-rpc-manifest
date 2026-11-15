# Security policy

Security fixes target the latest published versions. Report vulnerabilities
privately through [GitHub private vulnerability reporting](https://github.com/talaagent/protoc-gen-rpc-manifest/security/advisories/new).
If that form is unavailable, email **github@igm.pub**. Please do not disclose
an unpatched vulnerability in a public issue.

Include the affected crate and version, enabled features, OS, impact, and a
minimal reproduction. Relevant issues include generated code that a
crafted `.proto` comment or name can break out of (Rust, TypeScript or
JSON), the Connect caller sending headers or bodies somewhere other than
the configured base URL, credentials from header hooks leaking into error
messages or output, hangs or unbounded memory from a server's reply, and
panics on any of these. Remove tokens, headers and request bodies from
reports.
