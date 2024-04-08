pub enum Error {
    UnknownVersion { actual: u32 },
}

pub type Result<T> = std::result::Result<T, Error>;
