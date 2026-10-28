/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Adapt Shared Cargo Flags Into Library-Owned Build Settings.

#[derive(clap::Args)]
pub struct BuildArguments {
    #[arg(long, default_value = "Cargo.toml")]
    manifest_path: std::path::PathBuf,

    #[arg(short = 'p', long = "package")]
    packages: Vec<String>,

    #[arg(long)]
    target: Option<String>,

    #[arg(long)]
    target_dir: Option<std::path::PathBuf>,

    #[arg(long, default_value = "dev")]
    profile: harmos_packaging::config::BuildProfile,

    #[arg(long)]
    timings: bool,

    #[arg(long, value_delimiter = ',')]
    features: Vec<String>,

    #[arg(long)]
    no_default_features: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct CollectionArguments {
    #[arg(long, requires_all = ["metadata_json", "archive_json"])]
    from_cargo_json: Option<std::path::PathBuf>,

    #[arg(long, requires = "from_cargo_json")]
    metadata_json: Option<std::path::PathBuf>,

    #[arg(long, requires = "from_cargo_json")]
    archive_json: Option<std::path::PathBuf>,
}

impl BuildArguments {
    pub fn settings(self, backend: harmos_packaging::config::BuildBackend) -> harmos_packaging::config::BuildSettings {
        harmos_packaging::config::BuildSettings {
            backend,
            manifest: self.manifest_path,
            packages: self.packages,
            target: self.target,
            target_directory: self.target_dir,
            profile: self.profile,
            timings: self.timings,
            features: self.features,
            default_features: !self.no_default_features,
        }
    }
}

impl CollectionArguments {
    pub fn build(
        self, settings: &harmos_packaging::config::BuildSettings, cancellation: &harmos_packaging::BuildCancellation,
    ) -> Result<harmos_packaging::Build, harmos_packaging::Error> {
        match (self.from_cargo_json, self.metadata_json, self.archive_json) {
            (Some(messages), Some(metadata), Some(receipt)) => {
                if cancellation.is_cancelled() {
                    return Err(harmos_packaging::Error::Cancelled);
                }
                harmos_packaging::Build::from_cargo_outputs(
                    std::io::BufReader::new(std::fs::File::open(messages)?),
                    std::fs::File::open(metadata)?,
                    std::fs::File::open(receipt)?,
                    settings,
                )
            }
            (None, None, None) => harmos_packaging::build_with_cancellation(settings, cancellation),
            _ => Err(harmos_packaging::Error::Settings(
                "collection requires Cargo messages, metadata and the archive producer receipt".into(),
            )),
        }
    }
}
