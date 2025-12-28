// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
<!-- Note: Document from sync-markdown-to-rustdoc:start through sync-markdown-to-rustdoc:end
     is synchronized from README.md. Any changes to that range are not preserved. -->
<!-- tidy:sync-markdown-to-rustdoc:start -->

A library for tracking generated assemblies.

See [atomic-maybe-uninit#55](https://github.com/taiki-e/atomic-maybe-uninit/pull/55) for an usage example.

## Compatibility

All CPU architectures supported by Rust (x86, x86_64, Arm, AArch64, RISC-V, LoongArch, Arm64EC, s390x, MIPS, PowerPC, MSP430, AVR, SPARC, Hexagon, M68k, C-SKY, and Xtensa) have been confirmed to work as targets for generating assembly. Pull requests adding support for non-CPU architectures (such as GPU, WASM, BPF, etc.) are welcome.

x86_64 and AArch64 environments where all of the following commands are available are currently supported as host environments:

- `cargo`
- `docker`

<!-- tidy:sync-markdown-to-rustdoc:end -->
*/

#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_variables)
    )
))]
#![forbid(unsafe_code)]
#![warn(
    // Lints that may help when writing public library.
    missing_debug_implementations,
    // missing_docs,
    clippy::alloc_instead_of_core,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::impl_trait_in_params,
    // clippy::missing_inline_in_public_items,
    // clippy::std_instead_of_alloc,
    // clippy::std_instead_of_core,
)]
#![allow(clippy::missing_panics_doc)]

#[macro_use]
mod process;

mod cargo;
mod objdump;

use std::{
    env, fs,
    io::{self, IsTerminal as _, Write as _},
    path::{Path, PathBuf},
    process::Stdio,
};

use cargo_config2::TargetTripleRef;

use self::process::ProcessBuilder;

#[derive(Debug, Default)]
struct CommonConfig {
    cargo_args: Vec<String>,
    rustc_args: Vec<String>,
    objdump_args: Vec<String>,
    att_syntax: bool,
}

#[derive(Debug)]
#[must_use]
pub struct Revision {
    name: String,
    target: String,
    config: CommonConfig,
}

impl Revision {
    pub fn new<N: Into<String>, T: Into<String>>(name: N, target: T) -> Self {
        Self { name: name.into(), target: target.into(), config: CommonConfig::default() }
    }

    /// Adds additional command line arguments for `cargo`. (this revision only)
    ///
    /// This will be merged with the arguments passed via [`Tester::cargo_args`].
    ///
    /// See the output of `cargo rustc --help` for acceptable arguments.
    pub fn cargo_args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, args: I) -> Self {
        self.config.cargo_args.extend(args.into_iter().map(Into::into));
        self
    }
    /// Adds additional rustflags to set. (this revision only)
    ///
    /// This will be merged with rustflags passed via [`Tester::rustc_args`].
    pub fn rustc_args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, args: I) -> Self {
        self.config.rustc_args.extend(args.into_iter().map(Into::into));
        self
    }
    /// Adds additional command line arguments for objdump. (this revision only)
    ///
    /// This will be merged with the arguments passed via [`Tester::objdump_args`].
    pub fn objdump_args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, args: I) -> Self {
        self.config.objdump_args.extend(args.into_iter().map(Into::into));
        self
    }
    /// Use AT&T syntax in x86/x86_64 assemblies. (this revision only)
    ///
    /// By default, Intel syntax is used to match Rust inline assembly.
    pub fn att_syntax(mut self) -> Self {
        self.config.att_syntax = true;
        self
    }
}

#[derive(Debug)]
#[must_use]
pub struct Tester {
    config: CommonConfig,
}

impl Tester {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { config: CommonConfig::default() }
    }

    /// Dump assemblies for the given revisions.
    ///
    /// `dump_dir` is resolved to `manifest_dir.join(dump_dir)`.
    pub fn dump<M: AsRef<Path>, D: AsRef<Path>>(
        &self,
        manifest_dir: M,
        dump_dir: D,
        revisions: &[Revision],
    ) {
        dump(self, manifest_dir.as_ref(), dump_dir.as_ref(), revisions);
    }

    /// Adds additional command line arguments for `cargo`. (all revisions)
    ///
    /// This will be shared with all revisions.
    /// If you want to apply only to a specific revision, use [`Revision::cargo_args`] instead.
    ///
    /// See the output of `cargo rustc --help` for acceptable arguments.
    pub fn cargo_args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, args: I) -> Self {
        self.config.cargo_args.extend(args.into_iter().map(Into::into));
        self
    }
    /// Adds additional rustflags to set. (all revisions)
    ///
    /// This will be shared with all revisions.
    /// If you want to apply only to a specific revision, use [`Revision::rustc_args`] instead.
    pub fn rustc_args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, args: I) -> Self {
        self.config.rustc_args.extend(args.into_iter().map(Into::into));
        self
    }
    /// Adds additional command line arguments for objdump. (all revisions)
    ///
    /// This will be shared with all revisions.
    /// If you want to apply only to a specific revision, use [`Revision::objdump_args`] instead.
    pub fn objdump_args<I: IntoIterator<Item = S>, S: Into<String>>(mut self, args: I) -> Self {
        self.config.objdump_args.extend(args.into_iter().map(Into::into));
        self
    }
    /// Uses AT&T syntax in x86/x86_64 assemblies. (all revisions)
    ///
    /// This will be shared with all revisions.
    /// If you want to apply only to a specific revision, use [`Revision::att_syntax`] instead.
    ///
    /// By default, Intel syntax is used to match Rust inline assembly.
    pub fn att_syntax(mut self) -> Self {
        self.config.att_syntax = true;
        self
    }
}

fn dump(tester: &Tester, manifest_dir: &Path, dump_dir: &Path, revisions: &[Revision]) {
    let tcx = &TesterContext::new(tester, manifest_dir);
    let manifest_dir = Path::new(&tcx.manifest_path).parent().unwrap();
    let dump_dir = manifest_dir.join(dump_dir);
    let raw_dump_dir = tcx
        .metadata
        .target_directory
        .join("tests/asmtest/raw")
        .join(dump_dir.strip_prefix(manifest_dir).unwrap());

    let mut cargo_base_args = vec!["rustc", "--release", "--manifest-path", &tcx.manifest_path];
    let mut cargo_base_rest_args = vec!["--", "--emit=obj"];
    if !tcx.tester.config.cargo_args.is_empty() {
        let mut base_args = &mut cargo_base_args;
        for arg in &tcx.tester.config.cargo_args {
            if arg == "--" {
                base_args = &mut cargo_base_rest_args;
            } else {
                base_args.push(arg);
            }
        }
    }

    fs::create_dir_all(&dump_dir).unwrap();
    fs::create_dir_all(&raw_dump_dir).unwrap();
    for revision in revisions {
        eprintln!("testing revision {}", revision.name);
        // Get target info.
        let cfgs = cargo::print_cfg(&revision.target).unwrap();
        let target_name = TargetTripleRef::from(&revision.target);
        let target_name = target_name.triple();
        let target_arch = cfgs
            .lines()
            .find_map(|l| l.strip_prefix("target_arch=\""))
            .unwrap()
            .strip_suffix('"')
            .unwrap();
        let mut cx = RevisionContext {
            tcx,
            prefer_gnu: false, // TODO: make this an option
            revision,
            target_name,
            target_arch,
            is_x86_base: target_arch == "x86" || target_arch == "x86_64",
            is_hexagon: target_arch == "hexagon",
            obj_path: PathBuf::new(),
            verbose_function_names: vec![],
            out: String::new(),
        };

        // Build and handle messages from Cargo.
        cargo::build(&mut cx, &cargo_base_args, &cargo_base_rest_args);

        // Disassemble and handle output.
        let raw_out = objdump::disassemble(&mut cx);
        // Save raw assembly to target directory for debugging.
        fs::write(raw_dump_dir.join(revision.name.clone() + ".asm"), &raw_out).unwrap();
        objdump::handle_asm(&mut cx, &raw_out);

        // Check output.
        assert_diff(cx.tcx, dump_dir.join(revision.name.clone() + ".asm"), cx.out);
    }
}

struct TesterContext<'a> {
    tester: &'a Tester,
    // For Cargo
    manifest_path: String,
    config: cargo::Config,
    nightly: bool,
    metadata: cargo::Metadata,
    // For docker
    user: String,
}

impl<'a> TesterContext<'a> {
    fn new(tester: &'a Tester, manifest_dir: &Path) -> Self {
        // For Cargo
        let manifest_path = cargo::locate_project(&manifest_dir.join("Cargo.toml")).unwrap(); // Get the absolute path to the manifest.
        let metadata = cargo::metadata(&manifest_path).unwrap();
        let config = cargo::config(manifest_dir).unwrap();
        let rustc_version = config.rustc_version().unwrap();

        // For docker
        #[cfg(not(windows))]
        let user = {
            format!("{}:{}", rustix::process::getuid().as_raw(), rustix::process::getgid().as_raw())
        };
        #[cfg(windows)]
        let user = "1000:1000".to_owned();

        Self { tester, manifest_path, config, nightly: rustc_version.nightly, metadata, user }
    }

    fn docker_cmd(&self, workdir: &Path) -> ProcessBuilder {
        let mut volume = workdir.as_os_str().to_owned();
        volume.push(":");
        volume.push(workdir);
        cmd!(
            "docker",
            "run",
            "--rm",
            "--init",
            "-i",
            "--user",
            &self.user,
            "--volume",
            volume,
            "--workdir",
            workdir,
            "ghcr.io/taiki-e/objdump:binutils-2.45.1-llvm-21",
        )
    }
}

struct RevisionContext<'a> {
    tcx: &'a TesterContext<'a>,
    prefer_gnu: bool, // TODO: move to config
    revision: &'a Revision,
    target_name: &'a str,
    target_arch: &'a str,
    is_x86_base: bool,
    is_hexagon: bool,
    obj_path: PathBuf,
    verbose_function_names: Vec<&'a str>,
    out: String,
}

#[track_caller]
fn assert_diff(tcx: &TesterContext<'_>, expected_path: impl AsRef<Path>, actual: impl AsRef<[u8]>) {
    let actual = actual.as_ref();
    let expected_path = expected_path.as_ref();
    if !expected_path.is_file() {
        fs::create_dir_all(expected_path.parent().unwrap()).unwrap();
        fs::write(expected_path, "").unwrap();
    }
    let expected = fs::read(expected_path).unwrap();
    if expected != actual {
        if env::var_os("CI").is_some() {
            let color = if env::var_os("GITHUB_ACTIONS").is_some() || io::stdout().is_terminal() {
                &["-c", "color.ui=always"][..]
            } else {
                &[]
            };
            let mut child = tcx
                .docker_cmd(&env::current_dir().unwrap())
                .into_std()
                .arg("git")
                .arg("--no-pager")
                .args(color)
                .args(["diff", "--no-index", "--"])
                .arg(expected_path)
                .arg("-")
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.as_mut().unwrap().write_all(actual).unwrap();
            assert!(!child.wait().unwrap().success());
            panic!(
                "assertion failed; please run test locally and commit resulting changes, or apply the above diff as patch (e.g., `patch -p1 <<'EOF' ... EOF`)"
            );
        } else {
            fs::write(expected_path, actual).unwrap();
        }
    }
}
