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

use color_eyre::{eyre::bail, Result};

use config::{load, write_access_token};
use matrix::IndexerBot;

mod config;
mod indradb_utils;
mod matrix;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Errors in the config will crash directly.
    let config = load();

    // TODO: config which rewrites itself to have the data after login
    #[allow(clippy::unwrap_used)]
    let mut bot = match config.auth_data {
        config::AuthData::UsernamePassword(mxid, password) => {
            let bot = IndexerBot::new(
                config.homeserver_url,
                mxid,
                password,
                config.indradb_endpoint,
            )
            .await?;
            let access_token = bot.access_token();
            let device_id = bot.device_id();
            if access_token.is_none() || device_id.is_none() {
                bail!("Login to matrix must have failed. We got no access_token or device_id!");
            }
            write_access_token(access_token.unwrap(), device_id.unwrap())?;
            bot
        }
        config::AuthData::AccessToken(mxid, access_token, device_id) => {
            IndexerBot::relogin(
                config.homeserver_url,
                mxid,
                access_token,
                device_id,
                config.indradb_endpoint,
            )
            .await?
        }
    };
    bot.start_processing().await?;

    Ok(())
}
