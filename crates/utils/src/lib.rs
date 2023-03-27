#![deny(unsafe_code, clippy::unwrap_used)]
#![warn(
    clippy::cognitive_complexity,
    clippy::branches_sharing_code,
    clippy::imprecise_flops,
    clippy::missing_const_for_fn,
    clippy::mutex_integer,
    clippy::path_buf_push_overwrite,
    clippy::redundant_pub_crate,
    clippy::pedantic,
    clippy::dbg_macro,
    clippy::todo,
    clippy::fallible_impl_from,
    clippy::filetype_is_file,
    clippy::suboptimal_flops,
    clippy::fn_to_numeric_cast_any,
    clippy::if_then_some_else_none,
    clippy::imprecise_flops,
    clippy::lossy_float_literal,
    clippy::panic_in_result_fn,
    clippy::clone_on_ref_ptr
)]
#![allow(clippy::missing_panics_doc)]
// I am lazy. Dont blame me!
#![allow(missing_docs)]

use std::{collections::BTreeSet, fs::OpenOptions, path::PathBuf};

pub use indradb;
pub use indradb_proto;
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};

pub async fn get_client(
    endpoint: String,
) -> Result<indradb_proto::Client, indradb_proto::ClientError> {
    let mut client = indradb_proto::Client::new(endpoint.try_into()?).await?;
    client.ping().await?;
    Ok(client)
}

pub async fn get_client_retrying(
    endpoint: String,
) -> Result<indradb_proto::Client, indradb_proto::ClientError> {
    let mut retry_count = 10u8;
    let mut last_err = Option::<indradb_proto::ClientError>::None;

    while retry_count > 0 {
        match get_client(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(err) => {
                last_err = Some(err);
                if retry_count == 0 {
                    break;
                }
                sleep(Duration::from_secs(1)).await;
                retry_count -= 1;
            }
        }
    }

    Err(last_err.expect("We didnt get an error even though connection failed"))
}

fn get_identifier_config() -> PathBuf {
    let config_path = dirs::config_dir();
    if let Some(config_path) = config_path {
        config_path.join("knowledge-search/identifier.json")
    } else {
        panic!("System not supported.")
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct IdentifierConfig {
    identifiers: BTreeSet<String>,
}

pub fn get_identifier() -> Result<BTreeSet<String>, std::io::Error> {
    let identifier_file = OpenOptions::new()
        .read(true)
        .create(true)
        .open(get_identifier_config())?;
    let identifier_config: IdentifierConfig = serde_json::from_reader(identifier_file)?;
    Ok(identifier_config.identifiers)
}

pub fn add_identifiers(identifiers: &mut BTreeSet<String>) -> Result<(), std::io::Error> {
    let identifier_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(get_identifier_config())?;
    let mut identifier_config: IdentifierConfig = serde_json::from_reader(&identifier_file)?;
    identifier_config.identifiers.append(identifiers);
    serde_json::to_writer_pretty(identifier_file, &identifier_config)?;

    Ok(())
}
