use clap::{Parser, ValueHint};
use eyre::Result;
use foundry_compilers::remappings::Remapping;
use foundry_config::{
    figment,
    figment::{
        error::Kind::InvalidType,
        value::{Dict, Map, Value},
        Figment, Metadata, Profile, Provider,
    },
    find_project_root_path, remappings_from_env_var, Config,
};
use serde::Serialize;
use std::path::PathBuf;

/// Common arguments for a project's paths.
#[derive(Debug, Clone, Parser, Serialize, Default)]
#[clap(next_help_heading = "Project options")]
pub struct ProjectPathsArgs {
    #[clap(long)]
    #[serde(skip)]
    pub profile: Option<String>,
    /// The project's root path.
    ///
    /// By default root of the Git repository, if in one,
    /// or the current working directory.
    #[clap(long, value_hint = ValueHint::DirPath, value_name = "PATH")]
    #[serde(skip)]
    pub root: Option<PathBuf>,

    /// The contracts source directory.
    #[clap(long, short = 'C', value_hint = ValueHint::DirPath, value_name = "PATH")]
    #[serde(rename = "src", skip_serializing_if = "Option::is_none")]
    pub contracts: Option<PathBuf>,

    /// The project's remappings.
    #[clap(long, short = 'R')]
    #[serde(skip)]
    pub remappings: Vec<Remapping>,

    /// The project's remappings from the environment.
    #[clap(long, value_name = "ENV")]
    #[serde(skip)]
    pub remappings_env: Option<String>,

    /// The path to the compiler cache.
    #[clap(long, value_hint = ValueHint::DirPath, value_name = "PATH")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_path: Option<PathBuf>,

    /// The path to the library folder.
    #[clap(long, value_hint = ValueHint::DirPath, value_name = "PATH")]
    #[serde(rename = "libs", skip_serializing_if = "Vec::is_empty")]
    pub lib_paths: Vec<PathBuf>,

    /// Use the Hardhat-style project layout.
    ///
    /// This is the same as using: `--contracts contracts --lib-paths node_modules`.
    #[clap(long, conflicts_with = "contracts", visible_alias = "hh")]
    #[serde(skip)]
    pub hardhat: bool,

    /// Path to the config file.
    #[clap(long, value_hint = ValueHint::FilePath, value_name = "FILE")]
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
}

impl ProjectPathsArgs {
    /// Returns the root directory to use for configuring the [Project]
    ///
    /// This will be the `--root` argument if provided, otherwise see [find_project_root_path()]
    pub fn project_root(&self) -> PathBuf {
        self.root.clone().unwrap_or_else(|| find_project_root_path(None).unwrap())
    }

    /// Returns the remappings to add to the config
    pub fn get_remappings(&self) -> Vec<Remapping> {
        let mut remappings = self.remappings.clone();
        if let Some(env_remappings) =
            self.remappings_env.as_ref().and_then(|env| remappings_from_env_var(env))
        {
            remappings.extend(env_remappings.expect("Failed to parse env var remappings"));
        }
        remappings
    }
}

impl<'a> From<&'a ProjectPathsArgs> for Figment {
    fn from(args: &'a ProjectPathsArgs) -> Self {
        let profile = args.profile.clone().map(|p| p.into());
        if let Some(root) = args.root.clone() {
            Config::figment_with_root(root, profile)
        } else {
            Config::figment_with_root(find_project_root_path(None).unwrap(), profile)
        }
        .merge(args)
    }
}

impl<'a> From<&'a ProjectPathsArgs> for Config {
    fn from(args: &'a ProjectPathsArgs) -> Self {
        let figment: Figment = args.into();
        Config::from_provider(figment).sanitized()
    }
}

// Make this args a `figment::Provider` so that it can be merged into the `Config`
impl Provider for ProjectPathsArgs {
    fn metadata(&self) -> Metadata {
        Metadata::named("Project Paths Args Provider")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        let value = Value::serialize(self)?;
        let error = InvalidType(value.to_actual(), "map".into());
        let mut dict = value.into_dict().ok_or(error)?;

        let mut libs =
            self.lib_paths.iter().map(|p| format!("{}", p.display())).collect::<Vec<_>>();

        if self.hardhat {
            dict.insert("src".to_string(), "contracts".to_string().into());
            libs.push("node_modules".to_string());
        }

        if !libs.is_empty() {
            dict.insert("libs".to_string(), libs.into());
        }
        let profile = if let Some(profile) = self.profile.as_ref() {
            profile.into()
        } else {
            Config::selected_profile()
        };
        Ok(Map::from([(profile, dict)]))
    }
}
