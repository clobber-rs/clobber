// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! # Clobber
//!
//! Clobber is a moderation bot for matrix. Mainly intended for maintaining ACLs and providing some additional moderation functionality beyond what most matrix clients offer.
//!

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(missing_docs, missing_debug_implementations)]
#![warn(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]

use anyhow::Result;
use clobber::init;

#[tokio::main]
async fn main() -> Result<()> {
    crate::init().await?;
    Ok(())
}
