use anyhow::Result;
use ocipkg::ImageName;
use ommx::artifact::{media_types, Artifact};

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
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
