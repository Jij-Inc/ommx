use anyhow::Result;
use ommx::artifact::data_dir;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    // This example demonstrates that setting an invalid path will cause a panic
    // Run with: OMMX_LOCAL_REGISTRY_ROOT=/root/no_permission cargo run --example test_local_registry_panic
    
    println!("\n=== Test with invalid OMMX_LOCAL_REGISTRY_ROOT (should panic) ===");
    println!("Attempting to access data_dir with invalid path...");
    
    // This should panic if OMMX_LOCAL_REGISTRY_ROOT is set to an unwritable path
    let dir = data_dir()?;
    println!("Data dir: {:?}", dir);
    
    Ok(())
}