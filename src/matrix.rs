// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! Matrix-related functionality.

use crate::{
    config::{self, Config, SessionExt},
    PROGRAM_NAME, PROGRAM_VERSION,
};
use anyhow::Result;
use matrix_sdk::{reqwest, Client, ClientConfig, Session};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::io;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};
use tracing_attributes::instrument;

/// Struct containing info collected from user via interactive login, used for initial login.
#[derive(Debug)]
pub struct InteractiveLogin {
    /// Username
    pub username: String,
    /// Password
    pub password: String,
}

impl InteractiveLogin {
    /// Interactively collects login information from user via stdin
    pub fn from_stdin() -> Result<Self> {
        println!("Enter username: ");
        let mut username = String::new();
        io::stdin().read_line(&mut username)?;
        println!("Enter password: ");
        let password = rpassword::read_password_from_tty(None)?;
        Ok(Self {
            username: username.trim().to_owned(),
            password: password.trim().to_owned(),
        })
    }
}

/// Perform initial login with interactive login information collected from user
#[instrument]
pub async fn interactive_login() -> Result<Client> {
    debug!("Starting interactive login flow");
    let config = Config::read_config()?;
    // Set device display name to randomized string. Example: "Clobber_vzN2gq"
    let mut device_display_name = String::from("Clobber_");
    device_display_name.push_str(
        &rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(6)
            .map(char::from)
            .collect::<String>(),
    );
    // Interactively get login info from user
    let login = InteractiveLogin::from_stdin()?;
    let client = Client::new_with_config(
        reqwest::Url::parse(config.homeserver.url.as_str())?,
        client_config()?,
    )?;
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
    match session.save_session() {
        Ok(_) => debug!("Session saved successfully."),
        Err(e) => error!("Could not save session: {}", e),
    };
    Ok(client)
}

/// Restore login from saved session
#[instrument]
pub async fn login() -> Result<Client> {
    let client_config = client_config()?;
    let config = Config::read_config()?;
    let session = Session::load_session()?;
    let client = Client::new_with_config(
        reqwest::Url::parse(config.homeserver.url.as_str())?,
        client_config,
    )?;
    debug!("Restoring login from session.");
    client.restore_login(session).await?;
    Ok(client)
}

/// Construct `matrix_sdk` `ClientConfig`
fn client_config() -> Result<ClientConfig> {
    let client_config = ClientConfig::new()
        .user_agent(&format!("{}/{}", PROGRAM_NAME, PROGRAM_VERSION))?
        .store_path(config::get_data_dir()?);
    Ok(client_config)
}
