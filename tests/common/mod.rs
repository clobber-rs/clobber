// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie Graven <em@nao.sh>
// Licensed under the EUPL

use std::convert::TryFrom;

use matrix_sdk::reqwest::Url;
use matrix_sdk::ruma::api::client::r0::account::register::Request as RegistrationRequest;
use matrix_sdk::ruma::api::client::r0::uiaa::{AuthData, Dummy};
use matrix_sdk::ruma::assign;
use matrix_sdk::{Client, Session, SyncSettings};
use tokio::sync::OnceCell;
static CLIENT: OnceCell<Client> = OnceCell::const_new();

/// Sets up a  `matrix_sdk::Client` for use within integration tests. By making use of
/// `tokio::sync::OnceCell`, the first function call will set up a client and register and log in
/// to the CI embedded conduit instance, returning the client. Any subsequent calls will simply return the
/// client, already set up and ready to use.
pub async fn get_client() -> Client {
    CLIENT
        .get_or_init(|| async {
            let client = Client::new(Url::try_from("http://localhost:6167").unwrap()).unwrap();
            let mut request = assign!(RegistrationRequest::new(), {
                username: Some("clobber"),
                password: Some("foobar"),
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
            client
        })
        .await
        .clone()
}
