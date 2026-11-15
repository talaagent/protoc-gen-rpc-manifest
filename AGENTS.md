# protoc-gen-rpc-manifest: contributor and agent guide

A protoc/buf plugin that emits a manifest of RPC services and methods
(Rust, TypeScript, JSON), the `rpc-manifest` types and Connect JSON caller,
and the `rpc-manifest-cli` drop-in `rpc` subcommand, published together at
one version. Other projects depend on these crates, so public APIs and the
generated output stay small, documented and stable. CONTRIBUTING.md covers
the pull request flow and releases.

## Commands

Use the nix dev shell. It pins rustc (see `rust-toolchain.toml`) and
provides `protoc` and `buf` (the golden tests need `protoc`),
cargo-nextest and cargo-deny:

```sh
nix develop -c cargo fmt --all --check
nix develop -c cargo clippy --workspace --all-targets -- -D warnings
nix develop -c cargo clippy --workspace --all-targets --all-features -- -D warnings
nix develop -c cargo nextest run --workspace
nix develop -c cargo nextest run --workspace --all-features
nix develop -c cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" nix develop -c cargo doc --workspace --no-deps --all-features
nix develop -c cargo deny check
nix develop -c cargo publish --workspace --dry-run --allow-dirty
UPDATE_GOLDEN=1 nix develop -c cargo test -p protoc-gen-rpc-manifest --test golden
```

Every change must pass fmt, clippy (with and without `--all-features`),
the tests (default and `--all-features`), the doctests and the doc build.
Commit small, focused changes. The MSRV is the workspace `rust-version`
(1.85); CI's `msrv` job builds and tests each crate on it, so keep let
chains and other newer language features out.

## Layout

- `crates/protoc-gen-rpc-manifest`: `src/lib.rs` (`respond`/`generate`,
  `Error`), `options.rs` (plugin parameter), `model.rs` (descriptors →
  services: comments from `SourceCodeInfo` paths, aliases), `render.rs`
  (the three outputs, hand-written for stable bytes), `main.rs` (stdio).
  `tests/fixtures/` protos, `tests/golden/` expected outputs,
  `tests/golden.rs` compiles fixtures with `protoc` in the test. Its
  README.md is the root README (symlink).
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
  A change to the generated output is a user-visible change; say so in the
  commit subject.
- Library errors use thiserror with a stable `snake_case` `code()`;
  `miette` is an optional feature. anyhow only in tests.
- Don't name a method `code` on a type whose miette `Diagnostic` is
  derived with `transparent` wrappers: the derive calls `.code()` and
  picks the inherent one (see `rpc-manifest-cli`'s manual impl).
- No `unwrap` outside tests; `core`/`alloc` paths over `std` where they
  exist (clippy enforces both).
- The `rpc-manifest` and `rpc-manifest-cli` READMEs are their crate docs
  (`include_str!`), so their examples must compile and pass. Keep links in
  READMEs absolute so they work on crates.io.
- Changelog entries come from commit messages (release-plz), so write
  commit subjects that read well in CHANGELOG.md.
