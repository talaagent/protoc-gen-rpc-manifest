# protoc-gen-rpc-manifest

A protoc/buf plugin that writes a manifest of every RPC service and method
in your protos, and a small library so any CLI can list and call any RPC
generically over [ConnectRPC](https://connectrpc.com)'s JSON protocol. New
RPCs become callable from your admin CLI on the next `buf generate`, with
no CLI code changes and no `.proto` files needed at runtime.

| Crate | What |
| --- | --- |
| `protoc-gen-rpc-manifest` | The plugin binary. Rust, TypeScript or JSON output. |
| `rpc-manifest` | Manifest types and lookups; `connect` feature: generic Connect unary JSON caller. |
| `rpc-manifest-cli` | A drop-in `rpc` subcommand for bpaf (default) or clap. |

Not published to crates.io; depend on it by git.

## The plugin

Install it (`cargo install --git https://github.com/talaagent/protoc-gen-rpc-manifest protoc-gen-rpc-manifest`)
and add it to `buf.gen.yaml`:

```yaml
version: v2
plugins:
  - local: protoc-gen-rpc-manifest
    out: crates/admin/src/gen
    # One manifest for the whole module, not one per directory.
    strategy: all
    opt:
      - lang=rust
      - package=acme
      - alias=acme.user.v1.UserAdminService:uadmin
  - local: protoc-gen-rpc-manifest
    out: web/src/gen
    strategy: all
    opt: [lang=ts]
```

With protoc: `protoc --rpc-manifest_out=gen --rpc-manifest_opt=lang=json -I proto proto/acme/**/*.proto`.

Options (comma-separated, each repeatable where it makes sense):

| Option | Default | Meaning |
| --- | --- | --- |
| `lang=rust\|ts\|json` | `rust` | Output format; repeat for several files. |
| `package=<prefix>` | all | Keep services whose package is `<prefix>` or under it (`acme.user` keeps `acme.user.v1`, not `acme.username`). |
| `alias=<full.Service>:<alias>` | derived | Set a service's CLI alias. |
| `filename=<base>` | `rpc_manifest` | Output file name without extension; may contain `/`. |
| `crate=<path>` | `::rpc_manifest` | Path the Rust output uses for the types (e.g. a re-export). |

Only services in files selected for generation are listed, not those in
imports. Services are sorted by full name and methods keep declaration
order, so output is byte-for-byte stable. Default aliases drop a `Service`
suffix and kebab-case the rest (`HTTPInvoiceService` → `http-invoice`);
default aliases that collide get the last package segment appended
(`user-v1`, `user-v2`). An unknown option, an alias for a service that
isn't generated, or two services sharing an alias fail the generation.

Each service carries its full name, package, short name, alias and leading
comment; each method its name, input and output type full names,
`idempotency_level` (`unknown`, `no_side_effects`, `idempotent`), streaming
kind (`unary`, `client`, `server`, `bidi`) and leading comment. The
outputs for the test fixtures are in
[`crates/protoc-gen-rpc-manifest/tests/golden`](crates/protoc-gen-rpc-manifest/tests/golden):
Rust (`pub static SERVICES: &[rpc_manifest::Service]`), TypeScript
(`export const SERVICES: readonly RpcService[]`, camelCase keys) and JSON
(`{"services": [...]}`, snake_case keys).

## Embedding `rpc` in an admin CLI

```toml
[dependencies]
rpc-manifest = { git = "https://github.com/talaagent/protoc-gen-rpc-manifest" }
rpc-manifest-cli = { git = "https://github.com/talaagent/protoc-gen-rpc-manifest", features = ["tls"] }
```

```rust,ignore
use bpaf::Bpaf;
use rpc_manifest::Manifest;
use rpc_manifest::connect::{self, Client};
use rpc_manifest_cli::RpcArgs;

include!("gen/rpc_manifest.rs"); // pub static SERVICES

#[derive(Debug, Clone, Bpaf)]
enum Command {
    Rpc(#[bpaf(external(rpc_manifest_cli::rpc_command))] RpcArgs),
    // ...your other subcommands
}

async fn run_rpc(args: RpcArgs, base_url: &str, token: &str) -> Result<(), rpc_manifest_cli::Error> {
    let action = args.resolve(Manifest::new(SERVICES))?;
    // Listings and docs don't need a server; build the client lazily if
    // that's expensive (`action.needs_client()`).
    let client = Client::new(base_url)
        .hook(connect::bearer(token)?)
        .timeout(core::time::Duration::from_secs(30));
    action.run(&client, "myadmin rpc", &mut std::io::stdout().lock()).await
}
```

With clap, enable the `clap` feature and use `Rpc(RpcArgs)` in your
`#[derive(Subcommand)]` enum. Then:

```text
myadmin rpc                               # list services
myadmin rpc user                          # list UserService's methods
myadmin rpc user get-user                 # show GetUser's docs and types
myadmin rpc user GetUser '{"id":"7"}'     # call it; pretty-prints the reply
myadmin rpc user GetUser @req.json        # body from a file
echo '{}' | myadmin rpc user ListUsers -  # body from stdin
```

Names match case-insensitively with `-`/`_` ignored, and typos get
did-you-mean suggestions. A method with no body shows its docs instead of
calling it, so pass `'{}'` to call with an empty request. Streaming methods
are listed but can't be called (Connect unary JSON can't carry them).

A runnable version is
[`crates/rpc-manifest-cli/examples/admin.rs`](crates/rpc-manifest-cli/examples/admin.rs):
`cargo run -p rpc-manifest-cli --example admin -- rpc user`.

## The Connect caller

`rpc_manifest::connect::Client` POSTs `application/json` to
`<base>/<package.Service>/<Method>` with `Connect-Protocol-Version: 1`.

- Header hooks (`connect::bearer`, `connect::header`, or any
  `Fn(&mut HeaderMap) -> Result<(), BoxError>`) run before each call.
- `.timeout(d)` is enforced locally and sent as `Connect-Timeout-Ms`.
- Bring your own `reqwest::Client` with `Client::with_http`, e.g. one with
  `unix_socket(...)` to talk to a local server over a socket.
- Error replies decode into `ConnectError { code, message, details,
  http_status }`; non-Connect bodies get the code the spec infers from the
  HTTP status.
- Every error has a stable `snake_case` `code()`: the Connect code
  (`not_found`, `unauthenticated`, …) for server errors, else `timeout`,
  `transport`, `streaming_unsupported`, … Enable `miette` for
  `Diagnostic` impls.

## Alternatives

`buf curl` and `grpcurl` call RPCs ad hoc from schemas or reflection. This
is for embedding: short aliases, docs and validation baked into your own
binary, nothing else needed at runtime.

## Development

See [CLAUDE.md](CLAUDE.md). License: Apache-2.0.
