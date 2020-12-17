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
            message::MessageEventContent,
        },
        StrippedStateEvent, SyncMessageEvent,
    },
    EventEmitter, SyncRoom,
};
use std::time::Duration;

use tokio::time::delay_for;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

use crate::matrix::MatrixListener;

#[async_trait]
#[allow(unused_variables)]
impl EventEmitter for MatrixListener {
    async fn on_room_message(&self, room: SyncRoom, event: &SyncMessageEvent<MessageEventContent>) {
    }

    async fn on_stripped_state_member(
        &self,
        room: SyncRoom,
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
}

// Inspired by the AutoJoin example in matrix-rust-sdk
/// Handles incoming invites.
async fn accept_invite(
    listener: &MatrixListener,
    room: SyncRoom,
    room_member: &StrippedStateEvent<MemberEventContent>,
) {
    if let SyncRoom::Invited(room) = room {
        let room = room.read().await;
        if !listener
            .config
            .bot
            .allow_invites
            .iter()
            .any(|user| user == &room_member.sender)
        {
            info!(
                "Unauthorized user {} tried to invite bot to {}",
                &room_member.sender, &room.room_id
            );
            return;
        }
        debug!("Joining room: {}", room.room_id);
        let mut delay = 2;
        while let Err(e) = listener.client.join_room_by_id(&room.room_id).await {
            warn!(
                "Failed to join room: {} ({:?}), retrying in {}s",
                room.room_id, e, delay
            );

            delay_for(Duration::from_secs(delay)).await;
            delay *= 2;
            if delay > 3600 {
                error!("Couldn't join room {} ({:?})", room.room_id, e);
                break;
            }
        }
        info!("Joined room: {}", room.room_id);
    }
}
