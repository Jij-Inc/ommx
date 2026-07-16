//! Versioned production receivers for `ommx-pyo3-bridge`.
//!
//! Protocol implementations stay binding-private and out of the generated
//! Python API. Registering a new protocol alongside an existing one allows
//! their exact endpoint and payload interpretations to coexist.

mod v0;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    v0::register(module)
}
