// Clobber - a matrix moderation bot
// Copyright (C) 2020 em@nao.sh
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

//! # Clobber
//!
//! Clobber is a moderation bot for matrix. Mainly intended for maintaining ACLs and providing some additional moderation functionality beyond what most matrix clients offer.
//!



#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

mod config;

extern crate dirs_next as dirs;

use anyhow::Result;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use tracing::{warn, info, debug};

use matrix_sdk::{Client, ClientConfig};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Construct matrix_sdk ClientConfig
    let client_config = ClientConfig::new()
        .user_agent(format!("Clobber {}", env!("CARGO_PKG_VERSION")))
        .store_path(&config_dir.join("data"));

    // Initialize matrix_sdk Client
    let client = Client::new_with_config();

    Ok(())
}
