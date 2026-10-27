//! Command-line interface shared by the standalone executable and Python SDK.
//!
//! Enable the `cli` feature to use [`run`]. CLI argument parsing and bundled
//! plugin discovery belong here; Artifact operations remain with the Artifact SDK.
//! Plugin paths use a content-addressed cache, configurable via
//! `OMMX_PLUGIN_CACHE_DIR`, without opening the Artifact Local Registry.

use crate::artifact::{get_image_dir, Artifact};
use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use ocipkg::{oci_spec::image::ImageManifest, ImageName};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
};

mod agent_plugin;

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Show the version
    Version,

    /// Discover the bundled agent plugin
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },

    /// Discover bundled coding-agent skills
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },

    /// Login to the remote registry
    Login {
        /// Registry URL, e.g. https://ghcr.io/v2/Jij-Inc/ommx
        registry: String,
        /// Username
        #[clap(short, long)]
        username: Option<String>,
        /// Password
        #[clap(short, long)]
        password: Option<String>,
    },

    /// Show the image manifest as JSON
    Inspect {
        /// Container image name or the path of OCI archive
        image_name_or_path: String,
    },

    /// Push the image to remote registry
    Push {
        /// Path of OCI archive or the container image name stored in local registry
        image_name_or_path: String,
    },

    /// Pull the image from remote registry
    Pull {
        /// Container image name in remote registry
        image_name: String,
    },

    /// Load OCI archive into the local registry
    Load {
        /// Path of OCI archive or OCI directory
        path: PathBuf,
    },

    /// Save the image in the local registry to an OCI archive
    Save {
        /// Container image name
        image_name: String,
        /// Output file name of OCI archive
        output: PathBuf,
    },

    /// List the images in the local registry
    List,

    /// Get the directory where the image is stored
    ImageDirectory {
        /// Container image name
        image_name: String,
    },
}

#[derive(Subcommand)]
enum PluginCommand {
    /// Print the absolute directory path of the bundled OMMX plugin
    Path {
        /// Select one plugin instead of the default
        #[clap(value_enum)]
        name: Option<PluginName>,
    },
    /// Discover the plugin marketplace that lists the bundled plugin
    Marketplace {
        #[command(subcommand)]
        command: MarketplaceCommand,
    },
}

#[derive(Clone, ValueEnum)]
enum PluginName {
    Ommx,
}

#[derive(Subcommand)]
enum MarketplaceCommand {
    /// Print the absolute directory path of the bundled plugin marketplace
    Path,
}

#[derive(Subcommand)]
enum SkillCommand {
    /// Print the absolute skills directory path for linking or copying into a project
    Path {
        /// Print one skill directory instead of the skills parent directory
        #[clap(value_enum)]
        name: Option<SkillName>,
    },
}

#[derive(Clone, ValueEnum)]
enum SkillName {
    Ommx,
}

enum ImageNameOrPath {
    Local(ImageName),
    Remote(ImageName),
    OciArchive(PathBuf),
    OciDir(PathBuf),
}

impl ImageNameOrPath {
    fn parse(input: &str) -> Result<Self> {
        let path: &Path = input.as_ref();
        if path.is_dir() {
            return Ok(Self::OciDir(path.to_path_buf()));
        }
        if path.is_file() {
            return Ok(Self::OciArchive(path.to_path_buf()));
        }
        if let Ok(name) = ImageName::parse(input) {
            let path = get_image_dir(&name);
            if path.exists() {
                return Ok(Self::Local(name));
            } else {
                return Ok(Self::Remote(name));
            }
        }
        bail!("Invalid input: {}", input)
    }

    fn get_manifest(&self) -> Result<ImageManifest> {
        let manifest = match self {
            ImageNameOrPath::OciDir(path) => Artifact::from_oci_dir(path)?.get_manifest()?,
            ImageNameOrPath::OciArchive(path) => {
                Artifact::from_oci_archive(path)?.get_manifest()?
            }
            ImageNameOrPath::Local(name) => {
                let image_dir = get_image_dir(name);
                Artifact::from_oci_dir(&image_dir)?.get_manifest()?
            }
            ImageNameOrPath::Remote(name) => Artifact::from_remote(name.clone())?.get_manifest()?,
        };
        Ok(manifest)
    }
}

fn execute(command: Command) -> Result<()> {
    match &command {
        Command::Plugin { command } => {
            let root = agent_plugin::path()?;
            let path = match command {
                PluginCommand::Path { .. } => root.join("plugins/ommx"),
                PluginCommand::Marketplace {
                    command: MarketplaceCommand::Path,
                } => root,
            };
            writeln!(io::stdout(), "{}", path.display())?;
        }
        Command::Skill {
            command: SkillCommand::Path { name },
        } => {
            let root = agent_plugin::path()?.join("plugins/ommx/skills");
            let path = if name.is_some() {
                root.join("ommx")
            } else {
                root
            };
            writeln!(io::stdout(), "{}", path.display())?;
        }
        Command::Version => {
            writeln!(
                io::stdout(),
                "{:>12} {}",
                "Version".blue().bold(),
                built_info::PKG_VERSION,
            )?;
            writeln!(
                io::stdout(),
                "{:>12} {}",
                "Target".blue().bold(),
                built_info::TARGET,
            )?;
            if let Some(hash) = built_info::GIT_COMMIT_HASH {
                writeln!(io::stdout(), "{:>12} {}", "Git Commit".blue().bold(), hash)?;
            }
        }
        Command::Login {
            registry,
            username,
            password,
        } => {
            let url = url::Url::parse(registry)?;
            let mut auth = ocipkg::distribution::StoredAuth::load_all().unwrap_or_default();
            match (username, password) {
                (Some(username), Some(password)) => {
                    auth.add(url.domain().unwrap(), username, password);
                }
                (None, None) => {}
                _ => {
                    bail!("--username and --password must be provided at the same time");
                }
            }
            let _token = auth.get_token(&url)?;
            writeln!(io::stdout(), "Login succeed")?;

            auth.save()?;
        }

        Command::Inspect { image_name_or_path } => {
            let manifest = ImageNameOrPath::parse(image_name_or_path)?.get_manifest()?;
            // Manifest annotations use HashMaps. Keep output stable across
            // processes and Rust/Python dependency feature combinations.
            let mut manifest = serde_json::to_value(manifest)?;
            manifest.sort_all_objects();
            writeln!(io::stdout(), "{}", serde_json::to_string_pretty(&manifest)?)?;
        }

        Command::Push { image_name_or_path } => match ImageNameOrPath::parse(image_name_or_path)? {
            ImageNameOrPath::OciDir(path) => {
                let mut artifact = Artifact::from_oci_dir(&path)?;
                artifact.push()?;
            }
            ImageNameOrPath::OciArchive(path) => {
                let mut artifact = Artifact::from_oci_archive(&path)?;
                artifact.push()?;
            }
            ImageNameOrPath::Local(name) => {
                let image_dir = get_image_dir(&name);
                let mut artifact = Artifact::from_oci_dir(&image_dir)?;
                artifact.push()?;
            }
            ImageNameOrPath::Remote(name) => {
                bail!("Image not found in local: {}", name)
            }
        },

        Command::Pull { image_name } => {
            let name = ImageName::parse(image_name)?;
            let mut artifact = Artifact::from_remote(name)?;
            artifact.pull()?;
        }

        Command::Save { image_name, output } => {
            let name = ImageName::parse(image_name)?;
            let image_dir = get_image_dir(&name);
            let mut artifact = Artifact::from_oci_dir(&image_dir)?;
            artifact.save(output)?;
        }

        Command::Load { path } => {
            let mut artifact = Artifact::from_oci_archive(path)?;
            artifact.load()?;
        }

        Command::ImageDirectory { image_name } => {
            let name = ImageName::parse(image_name)?;
            let path = get_image_dir(&name);
            writeln!(io::stdout(), "{}", path.display())?;
        }

        Command::List => {
            for image_name in crate::artifact::get_images()? {
                writeln!(io::stdout(), "{image_name}")?;
            }
        }
    }
    Ok(())
}

/// Run the OMMX CLI with arguments including the executable name.
///
/// Prints command output and help to stdout, and errors to stderr. Returns
/// `0` on success (including help/version), `2` for invalid arguments, or `1`
/// when command execution or writing terminal output fails.
///
/// This function does not exit the process or replace its global logger.
///
/// ```no_run
/// let exit_code = ommx::cli::run(std::env::args_os());
/// ```
pub fn run<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let command = match Command::try_parse_from(args) {
        Ok(command) => command,
        Err(error) => {
            let exit_code = if error.use_stderr() { 2 } else { 0 };
            if let Err(error) = error.print() {
                let _ = writeln!(io::stderr(), "Error: {error}");
                return 1;
            }
            return exit_code;
        }
    };

    match execute(command) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr(), "Error: {error:?}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skill_path_parses_parent_and_named_paths() {
        for (args, expected) in [
            (vec!["ommx", "skill", "path"], None),
            (vec!["ommx", "skill", "path", "ommx"], Some("ommx")),
        ] {
            let Command::Skill {
                command: SkillCommand::Path { name },
            } = Command::try_parse_from(args).unwrap()
            else {
                panic!("expected skill path command");
            };
            assert_eq!(name.map(|SkillName::Ommx| "ommx"), expected);
        }
        assert!(Command::try_parse_from(["ommx", "skill"]).is_err());
        assert!(Command::try_parse_from(["ommx", "skill", "path", "../other"]).is_err());
        assert!(Command::try_parse_from(["ommx", "skill", "path", "--name", "ommx"]).is_err());
    }

    #[test]
    fn plugin_paths_follow_jijmodeling_command_syntax() {
        for args in [
            vec!["ommx", "plugin", "path"],
            vec!["ommx", "plugin", "path", "ommx"],
        ] {
            assert!(matches!(
                Command::try_parse_from(args).unwrap(),
                Command::Plugin {
                    command: PluginCommand::Path { .. }
                }
            ));
        }
        assert!(matches!(
            Command::try_parse_from(["ommx", "plugin", "marketplace", "path"]).unwrap(),
            Command::Plugin {
                command: PluginCommand::Marketplace {
                    command: MarketplaceCommand::Path
                }
            }
        ));
        for args in [
            vec!["ommx", "plugin"],
            vec!["ommx", "plugin", "marketplace"],
            vec!["ommx", "plugin", "path", "other"],
            vec!["ommx", "plugin", "path", "--name", "ommx"],
        ] {
            assert!(Command::try_parse_from(args).is_err());
        }
    }
}
