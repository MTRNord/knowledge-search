#[cfg(rocksdb)]
use std::path::Path;

pub use indradb;
pub use indradb_proto;
use tokio::time::{sleep, Duration};

pub async fn get_client() -> Result<indradb_proto::Client, indradb_proto::ClientError> {
    let mut client =
        indradb_proto::Client::new("grpc://127.0.0.1:27615".try_into().unwrap()).await?;
    client.ping().await?;
    Ok(client)
}

pub async fn get_client_retrying() -> Result<indradb_proto::Client, indradb_proto::ClientError> {
    let mut retry_count = 10u8;
    let mut last_err = Option::<indradb_proto::ClientError>::None;

    while retry_count > 0 {
        match get_client().await {
            Ok(client) => return Ok(client),
            Err(err) => {
                last_err = Some(err);
                if retry_count == 0 {
                    break;
                } else {
                    sleep(Duration::from_secs(1)).await;
                    retry_count -= 1;
                }
            }
        }
    }

    Err(last_err.unwrap())
}
