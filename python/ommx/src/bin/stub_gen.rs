use pyo3_stub_gen::Result;

fn main() -> Result<()> {
    let stub = _ommx_rust::stub_info()?;
    stub.generate()?;
    Ok(())
}
