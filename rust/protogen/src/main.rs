use anyhow::Result;
use prost_build::Config;
use std::path::Path;

fn main() -> Result<()> {
    let manifest_root: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let repo_root = manifest_root.join("../..").canonicalize()?;
    let proto_root = repo_root.join("protobuf");
    dbg!(&manifest_root, &repo_root, &proto_root);

    let protos = ["polynomial.proto"]
        .iter()
        .map(|p| proto_root.join(p))
        .collect::<Vec<_>>();

    let mut cfg = Config::new();
    cfg.out_dir(repo_root.join("rust/ommx/src"))
        .compile_protos(&protos, &[proto_root])?;
    Ok(())
}
