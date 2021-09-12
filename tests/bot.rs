mod common;

#[tokio::test]
async fn test() -> anyhow::Result<()> {
    let client = common::get_client().await;
    println!("{:?}", client.whoami().await?);
    Ok(())
}
