use pyo3_stub_gen::Result;
use std::{fs, path::Path, process::Command};

fn main() -> Result<()> {
    let stub = _ommx_rust::stub_info()?;
    stub.generate()?;

    let root: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    avoid_git_conflict_marker_headings(&root.join("../../docs/api/_items"))?;
    Command::new("ruff")
        .arg("format")
        .arg(root.join("ommx/__init__.py"))
        .arg(root.join("ommx/_ommx_rust/__init__.pyi"))
        .arg(root.join("ommx/artifact/__init__.py"))
        .arg(root.join("ommx/experiment/__init__.py"))
        .status()?;
    Ok(())
}

fn avoid_git_conflict_marker_headings(dir: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rst") {
            continue;
        }

        let content = fs::read_to_string(&path)?;
        let mut lines = content.splitn(3, '\n');
        let Some(title) = lines.next() else {
            continue;
        };
        let Some(underline) = lines.next() else {
            continue;
        };
        let Some(rest) = lines.next() else {
            continue;
        };

        // Git treats a standalone "=======" line in newly-added files as a
        // conflict marker. RST accepts longer title underlines.
        if underline == "=======" {
            fs::write(&path, format!("{title}\n========\n{rest}"))?;
        }
    }

    Ok(())
}
