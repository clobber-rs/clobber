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
            let request = assign!(RegistrationRequest::new(), {
                username: Some("clobber"),
                password: Some("foobar"),
                auth: Some(AuthData::direct_request("m.login.password"))
            });
            client.register(request).await.unwrap();
            client
        })
        .await
        .clone()
}
