// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{borrow::ToOwned as _, format, string::String, vec::Vec};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use build_context::{CARGO, RUSTC};
pub(crate) use cargo_config2::Config;
use serde_derive::Deserialize;

use crate::RevisionContext;

pub(crate) fn locate_project(manifest_path: &Path) -> Result<String> {
    cmd!(CARGO, "locate-project", "--message-format", "plain", "--manifest-path", manifest_path)
        .read()
}

pub(crate) fn metadata(manifest_path: &str) -> Result<Metadata> {
    let mut cmd = cmd!(
        CARGO,
        "metadata",
        "--format-version=1",
        "--no-deps",
        "--manifest-path",
        manifest_path
    );
    serde_json::from_str(&cmd.read()?).with_context(|| format!("failed to parse output from {cmd}"))
}

pub(crate) fn config(manifest_dir: &Path) -> Result<Config, cargo_config2::Error> {
    Config::load_with_options(
        manifest_dir,
        cargo_config2::ResolveOptions::default()
            .rustc(cargo_config2::PathAndArgs::new(RUSTC))
            .cargo(CARGO)
            .cargo_home(None)
            .host_triple(build_context::HOST),
    )
}

pub(crate) fn build(
    cx: &mut RevisionContext<'_>,
    cargo_base_args: &[&str],
    cargo_base_rest_args: &[&str],
) {
    let mut rustflags = cx.tcx.config.rustflags(&cx.revision.target).unwrap().unwrap_or_default();
    rustflags.push("-Z");
    rustflags.push("merge-functions=disabled");
    rustflags.flags.extend_from_slice(&cx.tcx.tester.config.rustc_args);
    rustflags.flags.extend_from_slice(&cx.revision.config.rustc_args);
    let rustflags = &rustflags.encode().unwrap();
    let mut args = cargo_base_args.to_owned();
    args.push("--target");
    args.push(&cx.revision.target);
    let mut rest_args = cargo_base_rest_args.to_owned();
    if !cx.revision.config.cargo_args.is_empty() {
        let mut base_args = &mut args;
        for arg in &cx.revision.config.cargo_args {
            if arg == "--" {
                base_args = &mut rest_args;
            } else {
                base_args.push(arg);
            }
        }
    }
    let mut cargo = cmd!(CARGO);
    if !cx.tcx.nightly {
        // We set -Z merge-functions=disabled to rustc.
        cargo.env("RUSTC_BOOTSTRAP", "1");
    }
    let Ok(json) = cargo
        .args(&args)
        .arg("--message-format=json")
        .args(&rest_args)
        .env("CARGO_ENCODED_RUSTFLAGS", rustflags)
        .read()
    else {
        // Show error from Cargo to the user.
        let mut cargo = cmd!(CARGO);
        if !cx.tcx.nightly {
            // We set -Z merge-functions=disabled to rustc.
            cargo.env("RUSTC_BOOTSTRAP", "1");
        }
        cargo.args(&args).args(&rest_args).env("CARGO_ENCODED_RUSTFLAGS", rustflags).run().unwrap();
        unreachable!()
    };
    let mut obj_path = None;
    'obj_path: for line in json.lines() {
        if line.trim_ascii_start().is_empty() {
            continue;
        }
        let artifact: Artifact = match serde_json::from_str(line) {
            Ok(artifact) => artifact,
            Err(_e) => continue,
        };
        if artifact.manifest_path != cx.tcx.manifest_path {
            continue;
        }
        if let Some(path) = artifact.obj_path() {
            obj_path = Some(path);
            break 'obj_path;
        }
    }
    let Some(obj_path) = obj_path else {
        panic!("not found .rmeta file in artifacts for {}", cx.tcx.manifest_path);
    };
    cx.obj_path = obj_path.canonicalize().expect("failed to canonicalize object file");
}

#[derive(Deserialize)]
pub(crate) struct Metadata {
    pub(crate) target_directory: PathBuf,
}

#[derive(Deserialize)]
struct Artifact {
    manifest_path: String,
    filenames: Vec<String>,
}

impl Artifact {
    fn obj_path(&self) -> Option<PathBuf> {
        self.filenames.iter().find_map(|filename| {
            let filename = Path::new(filename);
            let file_name = filename.file_name()?.to_str()?;
            let stem = file_name.strip_prefix("lib")?.strip_suffix(".rmeta")?;
            Some(filename.with_file_name(format!("{stem}.o")))
        })
    }
}

#[cfg(test)]
mod tests {
    use alloc::{borrow::ToOwned as _, string::String, vec};
    use std::path::Path;

    use super::Artifact;

    #[test]
    fn artifact_obj_path() {
        for (rmeta, obj) in [
            (
                "target/aarch64-unknown-linux-gnu/release/deps/libasm_test-0123456789abcdef.rmeta",
                "target/aarch64-unknown-linux-gnu/release/deps/asm_test-0123456789abcdef.o",
            ),
            (
                "target/aarch64-unknown-linux-gnu/release/build/asm-test/0123456789abcdef/out/libasm_test-0123456789abcdef.rmeta",
                "target/aarch64-unknown-linux-gnu/release/build/asm-test/0123456789abcdef/out/asm_test-0123456789abcdef.o",
            ),
        ] {
            let artifact =
                Artifact { manifest_path: String::new(), filenames: vec![rmeta.to_owned()] };
            assert_eq!(artifact.obj_path().unwrap(), Path::new(obj));
        }
    }
}
