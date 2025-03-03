#[derive(Debug)]
pub enum Error {
    UnsupportedV1Function,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::UnsupportedV1Function => write!(f, "Unsupported ommx.v1.Function is found. This is created by a newer version of OMMX SDK."),
        }
    }
}

impl std::error::Error for Error {}
