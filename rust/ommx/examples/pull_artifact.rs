use anyhow::Result;
use ocipkg::ImageName;
use ommx::artifact::media_types;
use ommx::experimental::artifact::Artifact;

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let image_name = ImageName::parse("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")?;

    // Pull the artifact from remote registry
    let mut artifact = Artifact::from_remote(image_name)?;
    artifact.pull()?;

    // Load the instance message from the artifact
    let layers = artifact.layers()?;
    for desc in layers.iter().filter(|d| {
        d.media_type()
            .as_ref()
            .map(|m| m == &media_types::v1_instance().to_string())
            .unwrap_or(false)
    }) {
        println!("{}", desc.digest());
    }
    Ok(())
}
