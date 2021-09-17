// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie Graven <em@nao.sh>
// Licensed under the EUPL

use std::convert::TryFrom;

use clobber::bot;
use clobber::config::{Bot, Config, Homeserver};
use matrix_sdk::reqwest::Url;
use matrix_sdk::room::Room;
use matrix_sdk::ruma::api::client::r0::account::register::Request as RegistrationRequest;
use matrix_sdk::ruma::api::client::r0::uiaa::{AuthData, Dummy};
use matrix_sdk::ruma::events::room::member::MemberEventContent;
use matrix_sdk::ruma::events::room::message::MessageEventContent;
use matrix_sdk::ruma::events::{StrippedStateEvent, SyncMessageEvent};
use matrix_sdk::ruma::{assign, UserId};
use matrix_sdk::{Client, Session, SyncSettings};
use tokio::sync::OnceCell;
static CLIENT: OnceCell<Client> = OnceCell::const_new();
static CLIENT_TARGET: OnceCell<Client> = OnceCell::const_new();

/// Sets up a  `matrix_sdk::Client` for use within integration tests. By making use of
/// `tokio::sync::OnceCell`, the first function call will set up a client and register and log in
/// to the CI embedded conduit instance, returning the client. Any subsequent calls will simply return the
/// client, already set up and ready to use.
pub async fn get_client() -> Client {
    CLIENT
        .get_or_init(|| async {
            let _ = tracing::subscriber::set_global_default(
                tracing_subscriber::fmt().pretty().finish(),
            );
            init_client("clobber").await
        })
        .await
        .clone()
}

/// Sets up another `matrix_sdk::Client` like `get_client()` to be used as a moderation target.
pub async fn get_client_target() -> Client {
    CLIENT_TARGET
        .get_or_init(|| async { init_client("target").await })
        .await
        .clone()
}

async fn init_client(username: &str) -> Client {
    let client = Client::new(Url::try_from("http://localhost:6167").unwrap()).unwrap();
    let mut request = assign!(RegistrationRequest::new(), {
        username: Some(username),
        password: Some("password"),
        inhibit_login: false
    });
    // Get UIAA session key
    let uiaa = match client.register(request.clone()).await {
        Err(e) => match e.uiaa_response().cloned() {
            Some(uiaa) => uiaa,
            None => panic!("Missing UIAA response 1"),
        },
        Ok(_) => {
            panic!("Missing UIAA response 2")
        }
    };
    // Set authentication data, m.login.dummy
    let dummy = assign!(Dummy::new(), {
        session: uiaa.session.as_deref()
    });
    request.auth = Some(AuthData::Dummy(dummy));
    let response = client.register(request).await.unwrap();
    let session = Session {
        access_token: response.access_token.unwrap(),
        user_id: response.user_id,
        device_id: response.device_id.unwrap(),
    };
    // Not entirely sure why this is necessary but it works ¯\_(ツ)_/¯
    client.restore_login(session).await.unwrap();
    client.sync_once(SyncSettings::new()).await.unwrap();

    let config = Config {
        homeserver: Homeserver {
            url: String::from("http://localhost:6167"),
        },
        bot: Bot {
            command_prefix: String::from("?"),
            allow_invites: vec![UserId::try_from("@clobber:localhost").unwrap()],
        },
    };

    client.register_event_handler(bot::on_room_member).await;
    client
        .register_event_handler({
            let config = config.clone();
            move |ev: SyncMessageEvent<MessageEventContent>, room: Room, client: Client| {
                let config = config.clone();
                async move { bot::on_room_message(ev, room, client, config).await }
            }
        })
        .await;
    client
        .register_event_handler({
            let config = config.clone();
            move |ev: StrippedStateEvent<MemberEventContent>, room: Room, client: Client| {
                let config = config.clone();
                async move { bot::on_stripped_state_member(ev, room, client, config).await }
            }
        })
        .await;
    client
}
