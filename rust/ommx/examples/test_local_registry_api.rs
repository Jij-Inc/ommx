use anyhow::Result;
use ommx::artifact::{data_dir, set_local_registry_root};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("\n=== Test API-based setting ===");
    
    // Set the local registry root via API
    println!("Setting local registry root to /tmp/ommx-test-api");
    set_local_registry_root("/tmp/ommx-test-api")?;
    
    println!("\n=== Verify data_dir returns the API-set value ===");
    println!("Data dir: {:?}", data_dir()?);
    
    println!("\n=== Try to set again (should fail) ===");
    match set_local_registry_root("/tmp/ommx-test-api2") {
        Ok(_) => println!("ERROR: Should have failed!"),
        Err(e) => println!("Expected error: {}", e),
    }
    
    Ok(())
}