//! The plugin's golden Rust output compiles against these types and the
//! lookups work on it. The golden is checked against the fixtures by the
//! plugin's own tests.

use rpc_manifest::{Idempotency, Manifest, Streaming};

include!("../../protoc-gen-rpc-manifest/tests/golden/rpc_manifest.rs");

const MANIFEST: Manifest = Manifest::new(SERVICES);

#[test]
fn generated_manifest_is_usable() {
    let aliases: Vec<_> = SERVICES.iter().map(|s| s.alias).collect();
    assert_eq!(aliases, ["http-invoice", "uadmin", "user"]);

    let (s, m) = MANIFEST.method("user", "rename-user").expect("lookup");
    assert_eq!(s.full_name, "acme.user.v1.UserService");
    assert_eq!(m.idempotency, Idempotency::Idempotent);
    assert_eq!(m.input_type, "acme.user.v1.RenameUserRequest");
    assert!(m.comments.contains("\"display name\""));
    assert_eq!(s.summary(), "Manages user accounts.");

    let (_, watch) = MANIFEST.method("user", "WatchUsers").expect("lookup");
    assert_eq!(watch.streaming, Streaming::Server);

    let err = MANIFEST.service("invoice").expect_err("no such alias");
    assert_eq!(err.suggestions(), ["http-invoice"]);
}
