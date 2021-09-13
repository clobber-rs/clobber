// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie Graven <em@nao.sh>
// Licensed under the EUPL

use anyhow::Result;
use clap::{App, Arg};
use matrix_sdk::{
    room::{Joined, Room},
    ruma::events::{
        room::{member::MemberEventContent, message::MessageEventContent},
        StrippedStateEvent, SyncMessageEvent,
    },
    Client, SyncSettings,
};
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

pub mod bot;
pub mod config;
pub mod matrix;

use crate::config::Config;

/// Name of the program, extracted from cargo environment variables.
pub const PROGRAM_NAME: &str = env!("CARGO_PKG_NAME");
/// Current version of the program.
pub const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Authors of the program.
pub const PROGRAM_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
/// Description of the program.
pub const PROGRAM_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

pub async fn init() -> Result<()> {
    tracing::subscriber::set_global_default(tracing_subscriber::fmt().pretty().finish())?;

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
    let config = Config::read_config()?;

    client.register_event_handler(bot::on_room_member).await;
    client
        .register_event_handler({
            let config = config.clone();
            move |ev: SyncMessageEvent<MessageEventContent>, room: Room, client: Client| {
                let config = config.clone();
                async move { bot::on_room_message(ev, room, client, config).await }
            }
        })
        .await;
    client
        .register_event_handler({
            let config = config.clone();
            move |ev: StrippedStateEvent<MemberEventContent>, room: Room, client: Client| {
                let config = config.clone();
                async move { bot::on_stripped_state_member(ev, room, client, config).await }
            }
        })
        .await;
    let settings = SyncSettings::default().token(client.sync_token().await.unwrap());
    // Sync until the end of ~time~
    client.sync(settings).await;
    Ok(())
}
