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
    events::{room::message::MessageEventContent, SyncMessageEvent},
    EventEmitter, SyncRoom,
};

use crate::matrix::MatrixListener;

#[async_trait]
#[allow(unused_variables)]
impl EventEmitter for MatrixListener {
    async fn on_room_message(&self, room: SyncRoom, event: &SyncMessageEvent<MessageEventContent>) {
    }
}
