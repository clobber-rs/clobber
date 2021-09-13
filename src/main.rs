// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie Graven <em@nao.sh>
// Licensed under the EUPL

//! # Clobber
//!
//! Clobber is a moderation bot for matrix. Mainly intended for maintaining ACLs and providing some additional moderation functionality beyond what most matrix clients offer.
//!

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]

#[tokio::main]
async fn main() -> Result<()> {
    crate::init()?;
    Ok(())
}
