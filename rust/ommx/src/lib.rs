//! Open Mathematics prograMming eXchange (OMMX)

pub(crate) mod ommx {
    pub(crate) mod v1 {
        include!("ommx.v1.rs");
    }
}

pub use ommx::v1::*;
