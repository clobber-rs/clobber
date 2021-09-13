use std::convert::TryFrom;

use matrix_sdk::ruma::UserId;

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
