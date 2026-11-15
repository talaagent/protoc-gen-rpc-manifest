# Contributing

Bug reports, documentation fixes, test cases and pull requests are welcome.
For a bug, include the crate versions, enabled features, protoc or buf
version, your OS, and the smallest `.proto` and plugin options or Rust code
that shows it. Open an issue first for anything large, especially a change
to the generated output. Report security issues privately (see
[SECURITY.md](SECURITY.md)).

Keep pull requests small and focused, with a commit subject that reads well
as a changelog line (release-plz builds CHANGELOG.md from them). Coding
conventions and the repository layout are in [AGENTS.md](AGENTS.md).

## Checks

The minimum Rust is 1.85. The Nix dev shell pins the toolchain CI uses and
provides `protoc` and `buf` (the golden tests run `protoc`), cargo-nextest
and cargo-deny; with rustup, `rust-toolchain.toml` picks the same
toolchain, and you need `protoc` on `PATH`.

```sh
nix develop -c cargo fmt --all --check
nix develop -c cargo clippy --workspace --all-targets --all-features -- -D warnings
nix develop -c cargo nextest run --workspace --all-features
nix develop -c cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" nix develop -c cargo doc --workspace --no-deps --all-features
nix develop -c cargo deny check
```

A change to the plugin's output updates the goldens with
`UPDATE_GOLDEN=1 nix develop -c cargo test -p protoc-gen-rpc-manifest --test golden`;
review the diff in `crates/protoc-gen-rpc-manifest/tests/golden/` and commit
it with the change.

CI also runs clippy and the tests without `--all-features`,
`rpc-manifest-cli` with clap instead of bpaf, each feature alone, each
crate built and tested on the MSRV, `cargo publish --workspace --dry-run`,
and the Nix flake. AGENTS.md lists the commands.

## Releases

All three crates share one version. `release-plz.toml` keeps them in one
version group, writes one root `CHANGELOG.md`, and has
`protoc-gen-rpc-manifest` own the `v<version>` tag and GitHub release. With
`release_always = false`, only a merged release PR publishes. Review the
version bump (cargo-semver-checks runs on the release PR) and the
changelog, and merge only after CI passes. release-plz publishes in
dependency order: rpc-manifest, then rpc-manifest-cli and
protoc-gen-rpc-manifest.

The workflow runs only on the public `talaagent` repository.

Release PRs use the built-in `GITHUB_TOKEN` by default, with job-level
`contents: write` and `pull-requests: write` permissions. Enable **Allow
GitHub Actions to create and approve pull requests** in the organization and
repository Actions settings, then set the repository variable
`RELEASE_PR=true`. The workflow dispatches CI for those PRs itself (with
`actions: write`), because events from `GITHUB_TOKEN` don't trigger
workflows. An optional `RELEASE_PLZ_TOKEN` secret (GitHub App token or PAT)
replaces the built-in token.

### First publication

Trusted publishing can't create a crate, so the first release of each crate
needs a token:

1. Create a short-lived crates.io token with the `publish-new` scope and
   store it as the `CARGO_REGISTRY_TOKEN` Actions secret.
2. Add the release date to the `0.1.0` heading in CHANGELOG.md.
3. Run the release-plz workflow by hand with `bootstrap=true`. It publishes
   the committed versions. `release_packages` can limit a run to a
   comma-separated subset, for example to publish rpc-manifest before the
   other two.
4. On crates.io, add a trusted publisher to every crate: owner `talaagent`,
   repository `protoc-gen-rpc-manifest`, workflow `release-plz.yml`, no
   environment.
5. Delete the `CARGO_REGISTRY_TOKEN` secret and revoke the token. Later
   releases use OIDC. A persistent token would override it.

Afterwards, check each crate's page on crates.io and its docs.rs build. A
crate added to the workspace later goes through the same steps.

## License

Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion in this project, as defined in the Apache-2.0
license, is licensed under Apache-2.0, without any additional terms or
conditions.
