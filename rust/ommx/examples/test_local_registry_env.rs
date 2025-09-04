use anyhow::Result;
use ommx::artifact::data_dir;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    // This example should be run with OMMX_LOCAL_REGISTRY_ROOT environment variable set
    println!("\n=== Test with OMMX_LOCAL_REGISTRY_ROOT env var ===");
    println!("Data dir: {:?}", data_dir()?);
    
    println!("\n=== Calling data_dir again should return the same cached value ===");
    println!("Data dir: {:?}", data_dir()?);
    
    Ok(())
}