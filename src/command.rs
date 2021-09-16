// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! Command functions and dispatch.

use matrix_sdk::{
    room::Joined,
    ruma::{
        events::{room::message::MessageEventContent, SyncMessageEvent},
        UserId,
    },
    Client,
};
use tracing::instrument;

use crate::{bot, config::Config};

/// Handles incoming commands and dispatches relevant functions.
#[instrument]
pub async fn handle_command(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    client: &Client,
    commands: Vec<&str>,
    config: &Config,
) -> anyhow::Result<()> {
    if commands.len() < 2 {
        help(event, room).await?;
        return Ok(());
    }
    let base_command = commands[1];
    let arguments = &commands[2..];
    match base_command {
        "help" => help(event, room).await?,
        "ban" => ban(event, room, client, config, arguments.to_vec()).await?,
        _ => unknown(event, room, config).await?,
    }
    Ok(())
}

/// Fallback when an unrecognized command is invoked.
#[instrument]
async fn unknown(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    config: &Config,
) -> anyhow::Result<()> {
    bot::send_reply(
        &format!("Unrecognized command, please try again or see {} help for available commands.", &config.bot.command_prefix),
        &format!("Unrecognized command, please try again or see <code>{} help</code> for available commands.", &config.bot.command_prefix),
        room,
        event.event_id.clone(),
    ).await?;
    Ok(())
}

/// Send help information.
#[instrument]
async fn help(event: &SyncMessageEvent<MessageEventContent>, room: &Joined) -> anyhow::Result<()> {
    let message = "There's nothing here yet";
    bot::send_reply(message, message, room, event.event_id.clone()).await?;
    Ok(())
}

/// Ban a user.
#[instrument]
async fn ban(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    client: &Client,
    config: &Config,
    args: Vec<&str>,
) -> anyhow::Result<()> {
    let (list, entity, reason) = match args.as_slice() {
        [list, entity] => (list, entity, None),
        [list, entity, reason] => (list, entity, Some(*reason)),
        _ => {
            let message = "Invalid number of arguments!";
            bot::send_reply(message, message, room, event.event_id.clone()).await?;
            return Ok(());
        }
    };
    bot::set_rule(
        client,
        &bot::get_list_room(client, list).await?,
        entity,
        bot::Action::Ban,
        reason,
    )
    .await?;
    Ok(())
}

/// Mute a user.
#[instrument]
async fn mute(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    client: &Client,
    config: &Config,
    args: Vec<&str>,
) -> anyhow::Result<()> {
    let (list, entity, reason) = match args.as_slice() {
        [list, entity] => (list, entity, None),
        [list, entity, reason] => (list, entity, Some(*reason)),
        _ => {
            let message = "Invalid number of arguments!";
            bot::send_reply(message, message, room, event.event_id.clone()).await?;
            return Ok(());
        }
    };
    bot::set_rule(
        client,
        &bot::get_list_room(client, list).await?,
        entity,
        bot::Action::Mute,
        reason,
    )
    .await?;
    Ok(())
}

// TODO: Implement kick command? What would it be useful for?
