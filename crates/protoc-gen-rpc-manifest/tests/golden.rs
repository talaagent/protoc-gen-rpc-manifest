//! Golden outputs from the fixture protos. Needs `protoc` on `PATH` (the
//! nix devShell has it). Regenerate with `UPDATE_GOLDEN=1 cargo test`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};
use prost::Message;
use prost_types::FileDescriptorSet;
use prost_types::compiler::CodeGeneratorRequest;
use prost_types::compiler::code_generator_response::File;

const SELECTED: &[&str] = &[
    "acme/billing/v1/billing.proto",
    "acme/internal/v1/debug.proto",
    "acme/user/v1/user.proto",
];

const OPTIONS: &str = "lang=rust,lang=ts,lang=json,package=acme.user,package=acme.billing,\
                       alias=acme.user.v1.UserAdminService:uadmin";

fn dir(sub: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(sub)
}

/// The fixtures (with imports and comments) as a descriptor set.
fn descriptors() -> Result<FileDescriptorSet> {
    let tmp = tempfile::tempdir()?;
    let out = tmp.path().join("set.pb");
    let status = Command::new("protoc")
        .arg("-I")
        .arg(dir("fixtures"))
        .args(["--include_imports", "--include_source_info"])
        .arg(format!("--descriptor_set_out={}", out.display()))
        .args(SELECTED)
        .status()
        .context("running protoc (run inside `nix develop`)")?;
    ensure!(status.success(), "protoc failed: {status}");
    Ok(FileDescriptorSet::decode(std::fs::read(out)?.as_slice())?)
}

fn request(set: FileDescriptorSet, parameter: &str) -> CodeGeneratorRequest {
    CodeGeneratorRequest {
        file_to_generate: SELECTED.iter().map(|s| (*s).into()).collect(),
        parameter: Some(parameter.into()),
        proto_file: set.file,
        ..CodeGeneratorRequest::default()
    }
}

fn check_golden(name: &str, actual: &str) -> Result<()> {
    let path = dir("golden").join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual)?;
        return Ok(());
    }
    let expected = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}; run with UPDATE_GOLDEN=1", path.display()))?;
    ensure!(
        expected == actual,
        "{name} differs from the golden; run with UPDATE_GOLDEN=1 and review the diff\n--- actual ---\n{actual}"
    );
    Ok(())
}

#[test]
fn goldens() -> Result<()> {
    let set = descriptors()?;
    let files = protoc_gen_rpc_manifest::generate(&request(set.clone(), OPTIONS))?;
    let names: Vec<_> = files.iter().map(File::name).collect();
    assert_eq!(
        names,
        ["rpc_manifest.rs", "rpc_manifest.ts", "rpc_manifest.json"]
    );
    for f in &files {
        check_golden(f.name(), f.content())?;
    }

    // No filter: every selected file's services, imports still excluded.
    let all = protoc_gen_rpc_manifest::generate(&request(set, "lang=json,filename=nested/all"))?;
    assert_eq!(all[0].name(), "nested/all.json");
    check_golden("all.json", all[0].content())
}

#[test]
fn input_order_does_not_matter() -> Result<()> {
    let set = descriptors()?;
    let a = protoc_gen_rpc_manifest::generate(&request(set.clone(), OPTIONS))?;
    let mut req = request(set, OPTIONS);
    req.proto_file.reverse();
    req.file_to_generate.reverse();
    let b = protoc_gen_rpc_manifest::generate(&req)?;
    assert_eq!(a, b);
    Ok(())
}

#[test]
fn errors_go_in_the_response() -> Result<()> {
    let set = descriptors()?;
    let res = protoc_gen_rpc_manifest::respond(&request(set, "alias=acme.user.v1.Nope:x"));
    assert!(res.file.is_empty());
    assert_eq!(res.error(), "alias for unknown service `acme.user.v1.Nope`");
    Ok(())
}

/// The real binary, driven by protoc, writes the same files.
#[test]
fn protoc_end_to_end() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let status = Command::new("protoc")
        .arg("-I")
        .arg(dir("fixtures"))
        .arg(format!(
            "--plugin=protoc-gen-rpc-manifest={}",
            env!("CARGO_BIN_EXE_protoc-gen-rpc-manifest")
        ))
        .arg(format!("--rpc-manifest_out={}", tmp.path().display()))
        .arg(format!("--rpc-manifest_opt={OPTIONS}"))
        .args(SELECTED)
        .status()?;
    ensure!(status.success(), "protoc failed: {status}");
    for name in ["rpc_manifest.rs", "rpc_manifest.ts", "rpc_manifest.json"] {
        let got = std::fs::read_to_string(tmp.path().join(name))?;
        let want = std::fs::read_to_string(dir("golden").join(name))?;
        assert_eq!(got, want, "{name}");
    }
    Ok(())
}
