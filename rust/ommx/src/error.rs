#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unknown version: {actual}")]
    UnknownVersion { actual: u32 },

    #[error(transparent)]
    Ocipkg(#[from] anyhow::Error),

    #[error(transparent)]
    OciSpec(#[from] ocipkg::oci_spec::OciSpecError),
}

pub type Result<T> = std::result::Result<T, Error>;
