// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! Bot functionality, command handling, etc.

use matrix_sdk::{
    room::{Joined, Room},
    ruma::events::{
        room::{
            member::{MemberEventContent, MembershipState},
            message::{
                InReplyTo, MessageEventContent, MessageType, Relation, TextMessageEventContent,
            },
        },
        AnyMessageEventContent, StrippedStateEvent, SyncMessageEvent, SyncStateEvent,
    },
    ruma::EventId,
    Client,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::instrument;

use tokio::time::sleep;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

use crate::config::Config;

/// Enum of available actions to apply to entity that matches rules.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Action {
    /// Ban the entity from the room.
    Ban,
}

/// Enum of available rule list event types.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum List {
    /// List of rules containing User IDs or globs.
    #[serde(rename = "sh.nao.list.user")]
    User {
        /// The user(s) the rule applies to.
        entity: String,
        /// The action to take on successful match.
        action: Action,
        /// User-supplied reason for creating the rule.
        reason: String,
    },
    /// List of rules containing server names or globs.
    #[serde(rename = "sh.nao.list.server")]
    Server {
        /// The server(s) the rule applies to.
        entity: String,
        /// The action to take on successful match.
        action: Action,
        /// User-supplied reason for creating the rule.
        reason: String,
    },
}

#[instrument]
pub async fn on_room_message(
    event: SyncMessageEvent<MessageEventContent>,
    room: Room,
    client: Client,
    config: Config,
) {
    if let Room::Joined(room) = room {
        // Match on m.text messages and get the message body
        let msg_body = if let SyncMessageEvent {
            content:
                MessageEventContent {
                    msgtype: MessageType::Text(TextMessageEventContent { body: msg_body, .. }),
                    ..
                },
            ..
        } = &event
        {
            info!("Matching on received message");
            msg_body
        } else {
            info!("Not matching on received message");
            return;
        };
        if msg_body
            .trim_start()
            .starts_with(&config.bot.command_prefix.to_string())
        {
            let mut words: Vec<&str> = msg_body.split(' ').collect();
            // Split prefix into separate word if 1 char long. don't judge ;-;
            if config.bot.command_prefix.chars().count() == 1 {
                words[0] = &words[0][1..];
                words.insert(0, &config.bot.command_prefix);
            }
            info!("Running command: {:?}", words);
            handle_command(&event, &room, words, &config).await.unwrap();
        }
    }
}

#[instrument]
pub async fn on_stripped_state_member(
    event: StrippedStateEvent<MemberEventContent>,
    room: Room,
    client: Client,
    config: Config,
) {
    // If `m.member` event is an invite and the bot is the invitee
    if event.content.membership == MembershipState::Invite
        && event.state_key == client.user_id().await.unwrap()
    {
        accept_invite(&event, &room, &client, &config)
            .await
            .unwrap();
    }
}

#[instrument]
pub async fn on_room_member(event: SyncStateEvent<MemberEventContent>, room: Room, client: Client) {
    if let Room::Joined(_room) = room {
        info!("Event handler firing");
    };
}

/// Handles incoming commands and dispatches relevant functions.
async fn handle_command(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    commands: Vec<&str>,
    config: &Config,
) -> Result<(), anyhow::Error> {
    if commands.len() < 2 {
        command_help(event, room).await?;
        return Ok(());
    }
    let base_command = commands[1];
    let _arguments = &commands[2..];
    match base_command {
        "help" => command_help(event, room).await?,
        _ => command_unknown(event, room, config).await?,
    }
    Ok(())
}

/// Fallback when an unrecognized command is invoked.
async fn command_unknown(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    config: &Config,
) -> Result<(), anyhow::Error> {
    send_reply(
        &format!("Unrecognized command, please try again or see {} help for available commands.", &config.bot.command_prefix),
        &format!("Unrecognized command, please try again or see <code>{} help</code> for available commands.", &config.bot.command_prefix),
        room,
        event.event_id.clone(),
    ).await?;
    Ok(())
}

/// Send help information.
#[instrument]
async fn command_help(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
) -> Result<(), anyhow::Error> {
    let message = "There's nothing here yet";
    send_reply(message, message, room, event.event_id.clone()).await?;
    Ok(())
}

/// Send `m.notice` reply to user.
async fn send_reply(
    plain: &str,
    html: &str,
    room: &Joined,
    event_id: EventId,
) -> Result<(), anyhow::Error> {
    let mut notice = MessageEventContent::notice_html(plain, html);
    notice.relates_to = Some(Relation::Reply {
        in_reply_to: InReplyTo::new(event_id),
    });
    let content = AnyMessageEventContent::RoomMessage(notice);
    room.send(content, None).await?;
    Ok(())
}

// Inspired by the AutoJoin example in matrix-rust-sdk
/// Handles incoming invites.
async fn accept_invite(
    event: &StrippedStateEvent<MemberEventContent>,
    room: &Room,
    client: &Client,
    config: &Config,
) -> Result<(), anyhow::Error> {
    if let Room::Invited(room) = room {
        if !config
            .bot
            .allow_invites
            .iter()
            .any(|user| user == &event.sender)
        {
            info!(
                "Unauthorized user {} tried to invite bot to {}",
                &event.sender,
                &room.room_id()
            );
            return Ok(());
        }
        // Keep trying to join the room we've been invited with exponential backoff until an hour
        // has passed.
        debug!("Joining room: {}", room.room_id());
        let mut delay = 2;
        while let Err(e) = client.join_room_by_id(room.room_id()).await {
            warn!(
                "Failed to join room: {} ({:?}), retrying in {}s",
                room.room_id(),
                e,
                delay
            );

            sleep(Duration::from_secs(delay)).await;
            delay *= 2;
            if delay > 3600 {
                error!("Couldn't join room {} ({:?})", room.room_id(), e);
                break;
            }
        }
        info!("Joined room: {}", room.room_id());
    }
    Ok(())
}
