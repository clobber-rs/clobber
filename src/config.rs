// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

use anyhow::Result;
use matrix_sdk::identifiers::UserId;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{fs, path::Path};
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

/// Top-level configuration struct.
#[derive(Deserialize, Serialize)]
pub struct Config {
    /// Homeserver-related configuration.
    pub homeserver: HomeserverConfig,
    /// Bot-related configuration.
    pub bot: BotConfig,
}

impl Config {
    /// Reads configuration from file.
    pub fn read_config() -> Result<Self> {
        let config_file = config_path();

        let data = fs::read(config_file)?;
        Ok(toml::from_slice(&data)?)
    }
}

/// Returns path to data directory, or creates one if one does not exist. Also checks directory permissions to ensure sensitive data is not readable by others.
pub fn get_data_dir() -> std::io::Result<PathBuf> {
    let data_dir = if Path::new("data").is_dir() {
        Path::new("data")
    } else if Path::new("/var/lib/clobber").is_dir() {
        Path::new("/var/lib/clobber")
    } else {
        Path::new("data")
    };
    debug!("Using '{:?}' as data directory", &data_dir);
    //let data_dir = dirs::data_dir().unwrap().join("clobber");

    if !data_dir.is_dir() {
        debug!("Creating data directory at {:?}", &data_dir);
        fs::create_dir(&data_dir)?;
        fs::set_permissions(&data_dir, PermissionsExt::from_mode(0o700))?;
    }
    // Do modulus to ignore setuid/setgid/sticky bits
    if data_dir.metadata()?.permissions().mode() % 0o1000 != 0o700 {
        warn!("Data directory has incorrect permissions set and may be readable by other users");
    }
    Ok(data_dir.to_path_buf())
}

/// Return the path to be used for reading the configurtion file
pub fn config_path() -> PathBuf {
    if Path::new("clobber.toml").is_file() {
        Path::new("clobber.toml")
    } else if Path::new("/etc/clobber/clobber.toml").is_file() {
        Path::new("/etc/clobber/clobber.toml")
    } else {
        Path::new("clobber.toml")
    }
    .to_path_buf()
}

/// Extension trait for matrix_sdk::Session. Provides convenience functions for loading and saving sessions.
pub trait SessionExt: Sized {
    /// Load session from file.
    fn load_session() -> Result<Self>;
    /// Save session to file.
    fn save_session(&self) -> Result<()>;
}

impl SessionExt for matrix_sdk::Session {
    fn load_session() -> Result<Self> {
        let data = fs::read(get_data_dir()?.join("session.json"))?;
        Ok(serde_json::from_slice(&data)?)
    }

    fn save_session(&self) -> Result<()> {
        let data = serde_json::to_string_pretty(&self)?;
        fs::write(get_data_dir()?.join("session.json"), data)?;
        Ok(())
    }
}

/// Homeserver-related configuration.
#[derive(Deserialize, Serialize, Default)]
pub struct HomeserverConfig {
    /// Homeserver URL
    pub url: String,
}

/// Bot-related configuration.
#[derive(Deserialize, Serialize, Default)]
pub struct BotConfig {
    /// Prefix used to invoke bot commands.
    pub command_prefix: String,
    /// Collection of users the bot will accept invites from.
    pub allow_invites: Vec<UserId>,
}
