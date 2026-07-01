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

        let underline_text = underline.trim_end_matches('\r');

        // Git treats a standalone "=======" line in newly-added files as a
        // conflict marker. RST accepts longer title underlines.
        if underline_text == "=======" {
            let line_ending = if underline.ends_with('\r') {
                "\r\n"
            } else {
                "\n"
            };
            let title = title.trim_end_matches('\r');
            fs::write(
                &path,
                format!("{title}{line_ending}========{line_ending}{rest}"),
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(name: &str) -> std::io::Result<PathBuf> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX_EPOCH")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ommx-stub-gen-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&dir)?;
        Ok(dir)
    }

    #[test]
    fn avoids_lf_conflict_marker_headings() -> std::io::Result<()> {
        let dir = temp_dir("lf")?;
        let path = dir.join("ommx.ToState.rst");
        fs::write(&path, "ToState\n=======\nbody\n")?;

        avoid_git_conflict_marker_headings(&dir)?;

        assert_eq!(fs::read_to_string(&path)?, "ToState\n========\nbody\n");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn avoids_crlf_conflict_marker_headings() -> std::io::Result<()> {
        let dir = temp_dir("crlf")?;
        let path = dir.join("ommx.ToState.rst");
        fs::write(&path, "ToState\r\n=======\r\nbody\r\n")?;

        avoid_git_conflict_marker_headings(&dir)?;

        assert_eq!(
            fs::read_to_string(&path)?,
            "ToState\r\n========\r\nbody\r\n"
        );
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}
