// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie Graven <em@nao.sh>
// Licensed under the EUPL

use std::{convert::TryFrom, time::Duration};

use matrix_sdk::{
    room::Room,
    ruma::{
        api::client::r0::room::create_room::Request,
        events::{room::message::MessageEventContent, AnyMessageEventContent},
        user_id, UserId,
    },
    SyncSettings,
};
use tokio::time::sleep;

use anyhow::anyhow;

mod common;

#[tokio::test]
async fn test() -> anyhow::Result<()> {
    let client = common::get_client().await;
    assert_eq!(
        client.whoami().await?.user_id,
        UserId::try_from("@clobber:localhost:6167").unwrap()
    );
    Ok(())
}

#[tokio::test]
async fn test_mute() -> anyhow::Result<()> {
    let client = common::get_client().await;
    let client_clone = client.clone();
    let sync = tokio::spawn(async move {
        let client = client_clone;
        client
            .sync(SyncSettings::default().token(client.sync_token().await.unwrap()))
            .await;
    });
    let client_target = common::get_client_target().await;
    let room = client.create_room(Request::new()).await?;
    client.join_room_by_id(&room.room_id).await?;
    let mut delay = 2;
    let room = loop {
        if let Some(room) = client.get_joined_room(&room.room_id) {
            break room;
        }
        sleep(Duration::from_secs(delay)).await;
        delay *= 2;

        if delay > 3600 {
            anyhow!("Joining room {} timed out", &room.room_id);
            panic!();
        }
    };
    room.invite_user_by_id(&user_id!("@target:localhost"))
        .await?;
    let target_room = client_target.get_room(room.room_id()).unwrap();
    if let Room::Invited(target_room) = target_room {
        let mut delay = 2;
        while target_room.accept_invitation().await.is_err() {
            sleep(Duration::from_secs(delay)).await;
            delay *= 2;

            if delay > 3600 {
                anyhow!("Joining room {} timed out", target_room.room_id());
                break;
            }
        }
    }

    let content = AnyMessageEventContent::RoomMessage(MessageEventContent::text_plain(
        "?mute @target:localhost",
    ));
    room.send(content, None).await.unwrap();
    let pl = room
        .get_member(&client_target.user_id().await.unwrap())
        .await?
        .unwrap()
        .power_level();
    assert_eq!(pl, -1);
    sync.abort();
    Ok(())
}
