use anyhow::{ensure, Context, Result};
use sha2::{Digest as _, Sha256};

pub fn sha256_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", encode_hex(&digest))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValidatedDigest<'a> {
    algorithm: &'a str,
    encoded: &'a str,
}

impl<'a> ValidatedDigest<'a> {
    pub(crate) fn parse(digest: &'a str) -> Result<Self> {
        let (algorithm, encoded) = digest
            .split_once(':')
            .with_context(|| format!("Digest must be '<algorithm>:<encoded>': {digest}"))?;
        ensure!(!algorithm.is_empty(), "Digest algorithm is empty");
        ensure!(!encoded.is_empty(), "Digest value is empty");
        ensure!(
            algorithm == "sha256",
            "Unsupported digest algorithm: {algorithm}"
        );
        ensure!(
            encoded.len() == 64 && encoded.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid sha256 digest: {digest}"
        );
        Ok(Self { algorithm, encoded })
    }

    pub(crate) fn algorithm(&self) -> &'a str {
        self.algorithm
    }

    pub(crate) fn encoded(&self) -> &'a str {
        self.encoded
    }
}

pub(crate) fn validate_digest(digest: &str) -> Result<()> {
    ValidatedDigest::parse(digest)?;
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
