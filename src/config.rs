// Clobber - a matrix moderation bot
// Copyright (C) 2020 em@nao.sh
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
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tracing::{warn, info, debug};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub bot: BotConfig,
}

impl Config {
    pub fn read_config() -> Result<Self> {
        let data = fs::read(init_config_dir()?)?;
        Ok(toml::from_slice(&data)?)
    }

    pub fn write_config(config: &Config) -> Result<()> {
        let content = toml::to_string_pretty(&config)?;
        fs::write(init_config_dir()?, content)?;

        Ok(())
    }

    pub fn new() -> Self {
        Self {
            bot: BotConfig::default(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct SessionStore {
    session: matrix_sdk::Session,
}

impl SessionStore {
    pub fn load_session(path: &Path) -> Result<matrix_sdk::Session> {
        let data = fs::read(path)?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn save_session(session: &matrix_sdk::Session, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(&session)?;
        fs::write(path, content)?;
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct BotConfig {
    pub command_prefix: String,
}

fn init_config_dir() -> Result<PathBuf> {
    // $XDG_CONFIG_HOME/clobber or $HOME/.config/clobber
    let config_dir = dirs::config_dir().unwrap().join("clobber");
    // If configuration directory does not exist, create directory and set appropriate permissions
    if !config_dir.is_dir() {
        debug!("Creating configuration directory at {}", config_dir.to_string_lossy());
        std::fs::create_dir(&config_dir)?;
        std::fs::set_permissions(&config_dir, Permissions::from_mode(0o700))?;
    }
    // Check configuration directory has correct permissions
    if !config_dir.metadata()?.permissions().mode() != 0o700 {
        warn!("Configuration directory has incorrect permissions set and may be readable by other users");
    }
    Ok(config_dir.join("config.toml"))
}
