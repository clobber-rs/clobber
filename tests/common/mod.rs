use std::convert::TryFrom;

use matrix_sdk::reqwest::Url;
use matrix_sdk::ruma::api::client::r0::account::register::Request as RegistrationRequest;
use matrix_sdk::ruma::api::client::r0::uiaa::AuthData;
use matrix_sdk::ruma::assign;
use matrix_sdk::Client;
use tokio::sync::OnceCell;
static CLIENT: OnceCell<Client> = OnceCell::const_new();

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
                    None => return Err(anyhow::anyhow!("Missing UIAA response")),
                },
                Ok(_) => {
                    return Err(anyhow::anyhow!("Missing UIAA response"));
                }
            };
            // Get the first step in the authentication flow (we're ignoring the rest)
            let stages = uiaa.flows.get(0);
            let kind = stages.and_then(|flow| flow.stages.get(0)).cloned();
            // Set authentication data, fallback to password type
            request.auth = Some(AuthData::DirectRequest {
                kind: kind.as_deref().unwrap_or("m.login.password"),
                session: uiaa.session.as_deref(),
                auth_parameters: Default::default(),
            });
            let response = client.register(request).await?;
            client.sync_once(SyncSettings::new()).await?;
            client
        })
        .await
        .clone()
}
