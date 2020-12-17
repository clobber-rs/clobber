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

//! Matrix-related functionality.

use crate::config::{Config, SessionExt};
use crate::{PROGRAM_NAME, PROGRAM_VERSION};
use anyhow::Result;
use matrix_sdk::reqwest::Url;
use matrix_sdk::{Client, ClientConfig, Session};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::io;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

/// Struct containing info collected from user via interactive login, used for initial login.
pub struct InteractiveLogin {
    /// Homeserver URL
    pub url: Url,
    /// Username
    pub username: String,
    /// Password
    pub password: String,
}

impl InteractiveLogin {
    /// Interactively collects login information from user via stdin
    pub fn from_stdin() -> Result<Self> {
        println!("Enter homeserver URL: ");
        let mut url = String::new();
        io::stdin().read_line(&mut url)?;
        let url = Url::parse(&url)?;
        println!("Enter username: ");
        let mut username = String::new();
        io::stdin().read_line(&mut username)?;
        println!("Enter password: ");
        let password = rpassword::read_password_from_tty(None)?;
        Ok(Self {
            url,
            username: username.trim().to_owned(),
            password: password.trim().to_owned(),
        })
    }
}

/// Perform initial login with interactive login information collected from user
pub async fn interactive_login() -> Result<Client> {
    debug!("Starting interactive login flow");
    // Set device display name to randomized string. Example: "Clobber_vzN2gq"
    let mut device_display_name = String::from("Clobber_");
    device_display_name.push_str(
        &rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(6)
            .collect::<String>(),
    );
    let client_config = client_config()?;
    // Interactively get login info from user
    let login = InteractiveLogin::from_stdin()?;
    let client = Client::new_with_config(login.url.clone(), client_config)?;
    let response = client
        .login(
            &login.username,
            &login.password,
            None,
            Some(&device_display_name),
        )
        .await;
    match &response {
        Ok(_) => info!("Logged in succesfully!"),
        Err(e) => error!("Login failed: {}", e),
    };
    let response = response?;
    let session = matrix_sdk::Session {
        access_token: response.access_token,
        user_id: response.user_id,
        device_id: response.device_id,
    };
    // Write session to file
    match session.save_session(&Config::get_data_dir()?.join("session.json")) {
        Ok(_) => debug!("Session saved successfully."),
        Err(e) => error!("Could not save session: {}", e),
    };
    Ok(client)
}

/// Restore login from saved session
pub async fn login() -> Result<Client> {
    let client_config = client_config()?;
    let config = Config::read_config()?;
    let session = Session::load_session()?;
    let client = Client::new_with_config(config.homeserver.url.as_str(), client_config)?;
    debug!("Restoring login from session.");
    client.restore_login(session).await?;
    Ok(client)
}

fn client_config() -> Result<ClientConfig> {
    // Construct matrix_sdk ClientConfig
    let client_config = ClientConfig::new()
        .user_agent(&format!("{}/{}", PROGRAM_NAME, PROGRAM_VERSION))?
        .store_path(Config::get_data_dir()?);
    Ok(client_config)
}

/// Listener struct for incoming matrix events
pub struct MatrixListener {
    /// Instance of config::Config
    pub config: Config,
    /// Instance of matrix_sdk::Client
    pub client: Client,
}

impl MatrixListener {
    /// Constructor for MatrixListener
    pub fn new(config: Config, client: Client) -> Self {
        Self { config, client }
    }
}
