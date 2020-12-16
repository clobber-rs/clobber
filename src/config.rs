// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use anyhow::Result;
use matrix_sdk::identifiers::UserId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub homeserver: HomeserverConfig,
    pub bot: BotConfig,
}

impl Config {
    pub fn read_config() -> Result<Self> {
        // $XDG_CONFIG_HOME/clobber or $HOME/.config/clobber
        let config_dir = dirs::config_dir().unwrap().join("clobber");
        // If configuration directory does not exist, create directory and set appropriate permissions
        if !config_dir.is_dir() {
            debug!(
                "Creating configuration directory at {}",
                config_dir.to_string_lossy()
            );
            fs::create_dir(&config_dir)?;
            fs::set_permissions(&config_dir, PermissionsExt::from_mode(0o700))?;
        }
        // Check configuration directory has correct permissions
        if config_dir.metadata()?.permissions().mode() % 0o1000 != 0o700 {
            warn!("Configuration directory has incorrect permissions set and may be readable by other users");
        }
        let config_file = config_dir.join("config.toml");
        if !config_file.is_file() {
            error!("No configuration file found! Creating empty example.");
            Config::new().write_config(&config_file)?;
            process::exit(1)
        }

        let data = fs::read(config_file)?;
        Ok(toml::from_slice(&data)?)
    }

    pub fn write_config(&self, path: &PathBuf) -> Result<()> {
        let content = toml::to_string_pretty(&self)?;
        fs::write(&path, content)?;

        Ok(())
    }

    pub fn get_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_dir().unwrap().join("clobber");

        if !data_dir.is_dir() {
            debug!("Creating data directory at {}", data_dir.to_string_lossy());
            fs::create_dir(&data_dir)?;
            fs::set_permissions(&data_dir, PermissionsExt::from_mode(0o700))?;
        }
        // Do modulus to ignore setuid/setgid/sticky bits
        if data_dir.metadata()?.permissions().mode() % 0o1000 != 0o700 {
            warn!(
                "Data directory has incorrect permissions set and may be readable by other users"
            );
        }
        Ok(data_dir)
    }

    pub fn new() -> Self {
        Self {
            bot: BotConfig::default(),
            homeserver: HomeserverConfig::default(),
        }
    }
}

pub trait SessionExt: Sized {
    fn load_session() -> Result<Self>;
    fn save_session(&self, path: &PathBuf) -> Result<()>;
}

impl SessionExt for matrix_sdk::Session {
    fn load_session() -> Result<Self> {
        let data = fs::read(Config::get_data_dir()?.join("session.json"))?;
        Ok(serde_json::from_slice(&data)?)
    }

    fn save_session(&self, path: &PathBuf) -> Result<()> {
        let data = serde_json::to_string_pretty(&self)?;
        fs::write(path, data)?;
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct BotConfig {
    pub command_prefix: String,
    pub allow_invites: Vec<UserId>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct HomeserverConfig {
    pub url: String,
}
