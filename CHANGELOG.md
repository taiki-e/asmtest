# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org).

Releases may yanked if there is a security bug, a soundness bug, or a regression.

<!--
Note: In this file, do not use the hard wrap in the middle of a sentence for compatibility with GitHub comment style markdown rendering.
-->

## [Unreleased]

- Improve demangling of AVR assembly with LLVM 22.

## [0.1.3] - 2026-01-14

- Fix panic when parsing MSP430 assembly.

- Improve demangling of big-endian PowerPC64 assembly.

- Enable [release immutability](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases).

## [0.1.2] - 2025-12-29

- Fix demangling issue on PowerPC64 assembly.

## [0.1.1] - 2025-12-28

- Remove the requirement for the `id` command.

- Documentation improvements.

## [0.1.0] - 2025-12-28

Initial release

[Unreleased]: https://github.com/taiki-e/asmtest/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/taiki-e/asmtest/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/taiki-e/asmtest/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/taiki-e/asmtest/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/taiki-e/asmtest/releases/tag/v0.1.0
