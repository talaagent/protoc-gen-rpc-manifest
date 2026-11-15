# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/talaagent/protoc-gen-rpc-manifest/releases/tag/v0.1.0) - 2026-11-15

First release. All crates share one version.

### Added

- `protoc-gen-rpc-manifest`: protoc/buf plugin writing a manifest of every
  RPC service and method in the files selected for generation, as Rust
  (`pub static SERVICES`), TypeScript (`export const SERVICES`) or JSON.
  Options `lang`, `package`, `alias`, `filename` and `crate`; services
  sorted by full name, methods in declaration order, byte-for-byte stable
  output; leading comments, idempotency level and streaming kind; derived
  kebab-case aliases with collision handling.
- `rpc-manifest`: the manifest types and `Manifest` lookups by full name or
  alias, with case- and separator-insensitive matching and did-you-mean
  suggestions. Features `serde`, `miette`, and `connect`/`tls`: a generic
  Connect unary JSON `Client` with header hooks (`bearer`, `header`),
  timeouts (`Connect-Timeout-Ms`), a bring-your-own `reqwest::Client`
  (e.g. over a unix socket), and coded `ConnectError`/`CallError`.
- `rpc-manifest-cli`: a drop-in `rpc` subcommand for bpaf (default) or clap
  that lists services and methods, shows a method's docs, and calls it with
  a JSON body from the command line, a file (`@path`) or stdin (`-`).
