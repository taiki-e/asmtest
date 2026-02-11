# asmtest

[![crates.io](https://img.shields.io/crates/v/asmtest?style=flat-square&logo=rust)](https://crates.io/crates/asmtest)
[![docs.rs](https://img.shields.io/badge/docs.rs-asmtest-blue?style=flat-square&logo=docs.rs)](https://docs.rs/asmtest)
[![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue?style=flat-square)](#license)
[![msrv](https://img.shields.io/badge/msrv-1.80-blue?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![github actions](https://img.shields.io/github/actions/workflow/status/taiki-e/asmtest/ci.yml?branch=main&style=flat-square&logo=github)](https://github.com/taiki-e/asmtest/actions)

<!-- tidy:sync-markdown-to-rustdoc:start:src/lib.rs -->

A library for tracking generated assemblies.

See [atomic-maybe-uninit#55](https://github.com/taiki-e/atomic-maybe-uninit/pull/55) for an usage example.

## Exclude generated assemblies from GitHub's language stats

<!-- Note: we cannot write inline gitattributes example due to https://github.com/rust-lang/rust/issues/86914 -->
By marking generated assemblies directory as `linguist-detectable=false` in `.gitattributes` ([example](https://github.com/taiki-e/atomic-maybe-uninit/blob/bf9b138a5676f31311defc652b9e6d975f44202d/.gitattributes#L5)), you can exclude generated assemblies from GitHub's language stats.

## Compatibility

All CPU architectures supported by Rust (x86, x86_64, Arm, AArch64, RISC-V, LoongArch, Arm64EC, s390x, MIPS, PowerPC, MSP430, AVR, SPARC, Hexagon, M68k, C-SKY, and Xtensa) have been confirmed to work as targets for generating assembly. Pull requests adding support for non-CPU architectures (such as GPU, WASM, BPF, etc.) are welcome.

x86_64 and AArch64 environments where all of the following commands are available are currently supported as host environments:

- `cargo`
- `docker`

<!-- tidy:sync-markdown-to-rustdoc:end -->

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
