use anyhow::Result;
use ommx::artifact::{data_dir, set_local_registry_root};
use std::env;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("\n=== Test 1: Default behavior (no env var, no API call) ===");
    println!("Data dir: {:?}", data_dir()?);
    
    println!("\n=== Test 2: Try to set via API after already initialized ===");
    match set_local_registry_root("/tmp/ommx-test-api") {
        Ok(_) => println!("ERROR: Should have failed!"),
        Err(e) => println!("Expected error: {}", e),
    }
    
    println!("\n=== Test 3: Verify data_dir returns the same value ===");
    println!("Data dir: {:?}", data_dir()?);
    
    Ok(())
}