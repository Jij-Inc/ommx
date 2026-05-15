use anyhow::Result;
use ommx::artifact::{
    local_registry::{pull_image, LocalRegistry},
    media_types, ImageRef, LocalArtifact,
};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let image_name = ImageRef::parse("ghcr.io/jij-inc/ommx/random_lp_instance:4303c7f")?;

    // Pull the artifact from the remote registry into the v3 SQLite
    // Local Registry, then open it for read by ref.
    let registry = LocalRegistry::shared_default()?;
    pull_image(registry, &image_name)?;
    let local = LocalArtifact::open_in_registry(registry, image_name)?;

    // Print the digest of each instance layer.
    for desc in local.layers()? {
        if desc.media_type() == &media_types::v1_instance() {
            println!("{}", desc.digest());
        }
    }
    Ok(())
}
