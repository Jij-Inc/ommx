// Generate the downstream fixture's stub independently from the Python SDK.
fn main() -> pyo3_stub_gen::Result<()> {
    ommx_pyo3_bridge_fixture::stub_info()?.generate()
}
