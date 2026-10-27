use pyo3::prelude::*;
#[cfg(feature = "stub_gen")]
use pyo3_stub_gen::{PyStubType, TypeInfo};
use std::ffi::OsString;

// Python's filesystem encoding preserves non-UTF-8 command-line paths.
#[derive(FromPyObject)]
#[pyo3(transparent)]
pub struct CliArgument(OsString);

#[cfg(feature = "stub_gen")]
impl PyStubType for CliArgument {
    fn type_output() -> TypeInfo {
        String::type_output()
    }
}

#[cfg_attr(feature = "stub_gen", pyo3_stub_gen::derive::gen_stub_pyfunction)]
#[pyfunction]
pub fn _run_cli(py: Python<'_>, args: Vec<CliArgument>) -> u8 {
    let args: Vec<_> = args.into_iter().map(|arg| arg.0).collect();
    py.detach(|| ommx::cli::run(args))
}
