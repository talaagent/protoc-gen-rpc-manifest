# protoc-gen-rpc-manifest

A protoc/buf plugin that emits a manifest of RPC services and methods
(Rust, TS, JSON), the `rpc-manifest` types + Connect JSON caller, and the
`rpc-manifest-cli` drop-in `rpc` subcommand. See README.md for usage.

## Commands

Run everything inside the nix devShell (it provides `protoc` and `buf`,
which the golden tests need):

```sh
nix develop -c cargo fmt
nix develop -c cargo clippy --all-targets --all-features -- -D warnings
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test --all-features
UPDATE_GOLDEN=1 nix develop -c cargo test -p protoc-gen-rpc-manifest --test golden
```

Every commit must pass fmt, clippy (both feature sets) and test. Commit
straight to `main`, small and focused; never force-push. Not published to
crates.io.

## Layout

- `crates/protoc-gen-rpc-manifest`: `src/lib.rs` (`respond`/`generate`,
  `Error`), `options.rs` (plugin parameter), `model.rs` (descriptors →
  services: comments from `SourceCodeInfo` paths, aliases), `render.rs`
  (the three outputs, hand-written for stable bytes), `main.rs` (stdio).
  `tests/fixtures/` protos, `tests/golden/` expected outputs,
  `tests/golden.rs` compiles fixtures with `protoc` in the test.
- `crates/rpc-manifest`: `manifest.rs` (types), `lookup.rs` (`Manifest`,
  loose matching, suggestions), `connect.rs` (feature `connect`: `Client`,
  `CallError`, `ConnectError`, `Code`). `tests/generated.rs` `include!`s
  the Rust golden to prove it compiles; `tests/connect.rs` runs the caller
  against an in-test axum server.
- `crates/rpc-manifest-cli`: `args.rs` (`RpcArgs` for bpaf/clap,
  `rpc_command`), `lib.rs` (`Action`, `run`, `Error`), `output.rs`
  (listings, docs, pretty JSON). `examples/admin.rs` is the README's
  embedding, runnable.

## Conventions

- The Rust golden is shared: changing the plugin's Rust output can break
  `rpc-manifest`'s and `rpc-manifest-cli`'s tests, which include it.
- Output must stay deterministic: services sorted by full name, methods in
  declaration order, no maps with random iteration order in rendering.
- Library errors use thiserror with a stable `snake_case` `code()`;
  `miette` is an optional feature. anyhow only in tests.
- Don't name a method `code` on a type whose miette `Diagnostic` is
  derived with `transparent` wrappers: the derive calls `.code()` and
  picks the inherent one (see `rpc-manifest-cli`'s manual impl).
