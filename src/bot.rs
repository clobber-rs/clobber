// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! Bot functionality, command handling, etc.

use async_trait::async_trait;
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
    EventHandler,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use tokio::time::sleep;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

use crate::matrix::Listener;

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

impl List {
    // TODO: Replace this with reading actual rule lists
    /// Placeholder function
    async fn mock_list() -> Self {
        Self::User {
            entity: String::from("@user:domain.tld"),
            action: Action::Ban,
            reason: String::from("COC Violation"),
        }
    }
}

#[async_trait]
impl EventHandler for Listener {
    async fn on_room_message(&self, room: Room, event: &SyncMessageEvent<MessageEventContent>) {
        if let Room::Joined(room) = room {
            // Match on m.text messages and get the message body
            let msg_body = if let SyncMessageEvent {
                content:
                    MessageEventContent {
                        msgtype: MessageType::Text(TextMessageEventContent { body: msg_body, .. }),
                        ..
                    },
                ..
            } = event
            {
                info!("Matching on received message");
                msg_body
            } else {
                info!("Not matching on received message");
                return;
            };
            if msg_body
                .trim_start()
                .starts_with(&self.config.bot.command_prefix)
            {
                let mut words: Vec<&str> = msg_body.split(' ').collect();
                // Split prefix into separate word if 1 char long. don't judge ;-;
                if self.config.bot.command_prefix.chars().count() == 1 {
                    words[0] = &words[0][1..];
                    words.insert(0, &self.config.bot.command_prefix);
                }
                info!("Running command: {:?}", words);
                handle_command(self, words, &room, event).await;
            }
        }
    }

    async fn on_stripped_state_member(
        &self,
        room: Room,
        room_member: &StrippedStateEvent<MemberEventContent>,
        _: Option<MemberEventContent>,
    ) {
        // If `m.member` event is an invite and the bot is the invitee
        if room_member.content.membership == MembershipState::Invite
            && room_member.state_key == self.client.user_id().await.unwrap()
        {
            accept_invite(self, room, room_member).await;
        }
    }

    async fn on_room_member(&self, _room: Room, room_member: &SyncStateEvent<MemberEventContent>) {
        // Check `invite`, `join` and `knock` states against rule lists and apply ban if there's a match
        if room_member.content.membership == MembershipState::Invite
            || room_member.content.membership == MembershipState::Join
            || room_member.content.membership == MembershipState::Knock
        {
            let list = List::mock_list().await;
            let entity = match list {
                List::User { entity, .. } | List::Server { entity, .. } => entity,
            };
            if entity == room_member.state_key {
                info!("Member event matched rule list");
            }
        }
    }

    async fn on_unrecognized_event(&self, _: Room, event: &serde_json::value::RawValue) {
        info!("Received unrecognized event: {:?}", event);
    }

    async fn on_custom_event(&self, _: Room, event: &matrix_sdk::CustomEvent<'_>) {
        info!("Received custom event: {:?}", event);
    }
}

/// Handles incoming commands and dispatches relevant functions.
async fn handle_command(
    listener: &Listener,
    commands: Vec<&str>,
    room: &Joined,
    event: &SyncMessageEvent<MessageEventContent>,
) {
    if commands.len() < 2 {
        return;
    }
    let base_command = commands[1];
    let _arguments = &commands[2..];
    match base_command {
        "help" => command_help(room, event).await,
        _ => command_unknown(listener, room, event).await,
    }
}

/// Fallback when an unrecognized command is invoked.
async fn command_unknown(
    listener: &Listener,
    room: &Joined,
    event: &SyncMessageEvent<MessageEventContent>,
) {
    send_reply(
        &format!("Unrecognized command, please try again or see {} help for available commands.", &listener.config.bot.command_prefix),
        &format!("Unrecognized command, please try again or see <code>{} help</code> for available commands.", &listener.config.bot.command_prefix),
        room,
        event.event_id.clone(),
    ).await;
}

/// Send help information.
async fn command_help(room: &Joined, event: &SyncMessageEvent<MessageEventContent>) {
    let message = "There's nothing here yet";
    send_reply(message, message, room, event.event_id.clone()).await;
}

/// Send `m.notice` reply to user.
async fn send_reply(plain: &str, html: &str, room: &Joined, event_id: EventId) {
    let mut notice = MessageEventContent::notice_html(plain, html);
    notice.relates_to = Some(Relation::Reply {
        in_reply_to: InReplyTo::new(event_id),
    });
    let content = AnyMessageEventContent::RoomMessage(notice);
    room.send(content, None).await.unwrap();
}

// Inspired by the AutoJoin example in matrix-rust-sdk
/// Handles incoming invites.
async fn accept_invite(
    listener: &Listener,
    room: Room,
    room_member: &StrippedStateEvent<MemberEventContent>,
) {
    if let Room::Invited(room) = room {
        if !listener
            .config
            .bot
            .allow_invites
            .iter()
            .any(|user| user == &room_member.sender)
        {
            info!(
                "Unauthorized user {} tried to invite bot to {}",
                &room_member.sender,
                &room.room_id()
            );
            return;
        }
        // Keep trying to join the room we've been invited with exponential backoff until an hour
        // has passed.
        debug!("Joining room: {}", room.room_id());
        let mut delay = 2;
        while let Err(e) = listener.client.join_room_by_id(room.room_id()).await {
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
}
