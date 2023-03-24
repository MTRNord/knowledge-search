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

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use color_eyre::Result;
use tera::Tera;
use thiserror::Error;
use tracing::info;
use utils::indradb_proto::Client;

mod algos;

const INDEX_TEMPLATE: &str = r#"
<form method="get" action="/results">
    <input name="query" value="" type="text" />
    <button type="submit" name="action" value="search">Search</button>
</form>
"#;

const RESULTS_TEMPLATE: &str = r#"
<h1>Event: {{ event_id }}</h1>
<p>{{ text_message_body }}</p>
<h3>Properties</h3>
<table>
    <tr>
        <th>name</th>
        <th>value</th>
    </tr>
    {% for prop in properties %}
        <tr>
            <td>{{ prop.0 }}</td>
            <td>{{ prop.1 }}</td>
        </tr>
    {% endfor %}
</table>
{% if room_id %}
    <h3>Linked room</h3>
    {% if room_name%}
    <h4>{{ room_name }}</h4>
    {% else %}
    <h4>{{ room_id }}</h4>
    {% endif %}

    {% if room_topic%}
    <p>{{ room_topic }}</p>
    {% endif %}
{% endif %}
"#;

pub struct AppState {
    tera: Tera,
    indradb: Client,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;
    let mut tera = Tera::default();
    tera.add_raw_templates(vec![("results.html", RESULTS_TEMPLATE)])?;
    tera.add_raw_templates(vec![("index.html", INDEX_TEMPLATE)])?;

    let indradb = utils::get_client_retrying("grpc://127.0.0.1:27615".to_string()).await?;

    let shared_state = Arc::new(AppState { tera, indradb });

    let app = Router::new()
        .route("/", get(index))
        .route("/results", get(results))
        .with_state(shared_state);

    // run it with hyper on localhost:3000
    info!("Opening server on 0.0.0.0:3000");
    axum::Server::bind(&"0.0.0.0:3000".parse()?)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

#[allow(clippy::unused_async)]
async fn results(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, AppError> {
    let query = params.get("query");
    if query.is_none() {
        return Err(AppError(color_eyre::eyre::eyre!("Missing query parameter")));
    }
    let mut context = tera::Context::new();
    context.insert("text_message_body", "Blub");
    context.insert("event_id", "$abc");
    context.insert("properties", &[("a", "b")]);
    context.insert("room_id", "!abc");
    let rendered = state.tera.render("results.html", &context)?;

    Ok(Html(rendered))
}

#[allow(clippy::unused_async)]
async fn index(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let context = tera::Context::new();
    let rendered = state.tera.render("index.html", &context)?;

    Ok(Html(rendered))
}

/// Error wrapper for [`tera::Error`]
#[derive(Error, Debug)]
pub enum TeraError {
    /// See [`tera::Error`]
    #[error(transparent)]
    RenderError(#[from] tera::Error),
}

impl IntoResponse for TeraError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}

// Make our own error that wraps `color_eyre::Error`.
struct AppError(color_eyre::Report);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, color_eyre::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<color_eyre::Report>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
