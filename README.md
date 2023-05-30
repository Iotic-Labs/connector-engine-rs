# Iotics Connector Engine in Rust

[![CI][ci_badge]][ci]

## Overview

A framework that greatly simplifies the process of writing high-performance, scalable and resilient connectors for IOTICS.

## Prerequisites

-   [Rust][toolchain]
-   [cargo-make][cargo-make]
-   [Golang][golang]
-   [Clang][clang]

## Usage

Without TLS

```bash
iotics-connector-engine = { git = "https://github.com/Iotic-Labs/connector-engine-rs.git" }
```

With TLS

```bash
iotics-connector-engine = { git = "https://github.com/Iotic-Labs/connector-engine-rs.git", features = ["tls"] }
```

## Examples

TODO

## Tutorial

TODO

[ci_badge]: https://github.com/Iotic-Labs/iotics-connector-engine/workflows/CI/badge.svg?branch=main
[ci]: https://github.com/Iotic-Labs/iotics-connector-engine/actions
[toolchain]: https://rustup.rs
[cargo-make]: https://github.com/sagiegurari/cargo-make
[golang]: https://golang.org/doc/install
[clang]: https://clang.llvm.org/get_started.html

### OSS Review

```
  cargo install cargo-license
  cargo license -t --avoid-build-deps --avoid-dev-deps --direct-deps-only
```
