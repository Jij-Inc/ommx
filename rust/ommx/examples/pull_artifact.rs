use anyhow::Result;
use ocipkg::ImageName;
use ommx::artifact::{media_types, Artifact};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")?;

    // Pull the artifact from remote registry
    let mut remote = Artifact::from_remote(image_name)?;
    let mut local = remote.pull()?;

    // Load the instance message from the artifact
    for desc in local.get_layer_descriptors(&media_types::v1_instance())? {
        println!("{}", desc.digest());
    }
    Ok(())
}
