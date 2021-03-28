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

//! Command handling and other bot functionality.

use async_trait::async_trait;
use matrix_sdk::{
    events::{
        room::{
            member::{MemberEventContent, MembershipState},
            message::{MessageEventContent, MessageType, Relation, TextMessageEventContent},
            relationships::InReplyTo,
        },
        AnyMessageEventContent, StrippedStateEvent, SyncMessageEvent,
    },
    identifiers::EventId,
    room::{Joined, Room},
    Client, EventHandler,
};
use std::time::Duration;

use tokio::time::sleep;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

use crate::matrix::MatrixListener;

#[async_trait]
impl EventHandler for MatrixListener {
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
                handle_command(&self, words, &room, &event).await;
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
            accept_invite(&self, room, &room_member).await;
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
    listener: &MatrixListener,
    commands: Vec<&str>,
    room: &Joined,
    event: &SyncMessageEvent<MessageEventContent>,
) {
    if commands.len() < 2 {
        return;
    }
    let base_command = commands[1];
    let arguments = &commands[2..];
    match base_command {
        "help" => command_help(&room, &event).await,
        _ => command_unknown(&listener, &room, &event).await,
    }
}

/// Fallback when an unrecognized command is invoked.
async fn command_unknown(
    listener: &MatrixListener,
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
        in_reply_to: InReplyTo { event_id },
    });
    let content = AnyMessageEventContent::RoomMessage(notice);
    room.send(content, None).await.unwrap();
}

// Inspired by the AutoJoin example in matrix-rust-sdk
/// Handles incoming invites.
async fn accept_invite(
    listener: &MatrixListener,
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
        while let Err(e) = listener.client.join_room_by_id(&room.room_id()).await {
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
