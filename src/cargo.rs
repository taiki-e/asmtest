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
    let mut hash = None;
    'hash: for line in json.lines() {
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
        for filename in &artifact.filenames {
            if let Some(f) = filename.strip_suffix(".rmeta") {
                if let Some((_, h)) = f.rsplit_once('-') {
                    hash = Some((h.to_owned(), artifact));
                    break 'hash;
                }
            }
        }
    }
    let Some((hash, artifact)) = hash else {
        panic!("not found .rmeta file in artifacts for {}", cx.tcx.manifest_path);
    };
    // TODO: search both?
    cx.obj_path = cx
        .tcx
        .metadata
        .build_directory
        .as_ref()
        .unwrap_or(&cx.tcx.metadata.target_directory)
        .canonicalize()
        .expect("failed to canonicalize target directory")
        .join(cx.target_name)
        .join("release/deps")
        .join(format!(
            "{}-{hash}.o",
            Path::new(&artifact.package_id)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .split_once('#')
                .context(artifact.package_id.clone())
                .unwrap()
                .0
                .replace('-', "_")
        ));
}

#[derive(Deserialize)]
pub(crate) struct Metadata {
    pub(crate) target_directory: PathBuf,
    pub(crate) build_directory: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Artifact {
    package_id: String,
    manifest_path: String,
    filenames: Vec<String>,
}
