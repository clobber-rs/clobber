// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! # Clobber
//!
//! Clobber is a moderation bot for matrix. Mainly intended for maintaining ACLs and providing some additional moderation functionality beyond what most matrix clients offer.
//!

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

pub mod bot;
pub mod config;
pub mod matrix;

extern crate clap;
extern crate dirs_next as dirs;

use anyhow::Result;
use clap::{App, Arg};
use matrix_sdk::SyncSettings;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

use crate::{config::Config, matrix::MatrixListener};

/// Name of the program, extracted from cargo environment variables.
pub const PROGRAM_NAME: &str = env!("CARGO_PKG_NAME");
/// Current version of the program.
pub const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Authors of the program.
pub const PROGRAM_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
/// Description of the program.
pub const PROGRAM_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = App::new(PROGRAM_NAME)
        .version(PROGRAM_VERSION)
        .author(PROGRAM_AUTHORS)
        .about(PROGRAM_DESCRIPTION)
        .arg(
            Arg::with_name("login")
                .short("l")
                .long("login")
                .help("Starts interactive login"),
        )
        .get_matches();
    let client = if args.is_present("login") {
        // Login flag supplied, perform interactive login
        matrix::interactive_login().await?
    } else {
        // No login flag supplied, restore login from session
        match matrix::login().await {
            Ok(client) => {
                info!("Successfully restored login from session");
                client
            }
            Err(e) => {
                error!("Could not restore login: {}", e);
                return Err(e);
            }
        }
    };
    client.sync_once(SyncSettings::default()).await?;
    let listener = MatrixListener::new(Config::read_config()?, client.clone());

    client.set_event_handler(Box::new(listener)).await;
    let settings = SyncSettings::default().token(client.sync_token().await.unwrap());
    // Sync until the end of ~time~
    client.sync(settings).await;

    Ok(())
}
