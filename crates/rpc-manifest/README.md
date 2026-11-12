# rpc-manifest

The types behind `protoc-gen-rpc-manifest lang=rust` output: [`Service`],
[`Method`], [`Idempotency`], [`Streaming`], plus [`Manifest`] lookups by
full name or alias with forgiving matching and did-you-mean suggestions.

With the `connect` feature, `connect::Client` calls any unary method
generically with Connect's JSON protocol. `tls` adds rustls for `https://`.
`serde` derives `Serialize` for the manifest types; `miette` derives
`Diagnostic` for the errors.

```rust
use rpc_manifest::{Idempotency, Manifest, Method, Service, Streaming};

static SERVICES: &[Service] = &[Service {
    full_name: "acme.user.v1.UserService",
    package: "acme.user.v1",
    name: "UserService",
    alias: "user",
    comments: "Manages users.",
    methods: &[Method {
        name: "GetUser",
        input_type: "acme.user.v1.GetUserRequest",
        output_type: "acme.user.v1.GetUserResponse",
        idempotency: Idempotency::NoSideEffects,
        streaming: Streaming::Unary,
        comments: "Fetches one user.",
    }],
}];

let manifest = Manifest::new(SERVICES);
let (service, method) = manifest.method("user", "get-user").unwrap();
assert_eq!((service.full_name, method.name), ("acme.user.v1.UserService", "GetUser"));

let err = manifest.service("usr").unwrap_err();
assert_eq!(err.code(), "unknown_service");
assert_eq!(err.suggestions(), ["user"]);
```

In real use, generate the static and `include!` it:

```rust,ignore
include!(concat!(env!("OUT_DIR"), "/rpc_manifest.rs"));
// or, checked in: include!("gen/rpc_manifest.rs");
```
