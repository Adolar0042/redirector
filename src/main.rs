use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse};
use axum::routing::post;
use axum::{Json, Router, extract::Query, response::Redirect, routing::get};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use heck::ToTitleCase;
use redirector::cli::SubCommand::Completions;
use redirector::cli::{Cli, SubCommand};
use redirector::config::{AppState, append_file_config, get_file_config};
use redirector::{BANG_CACHE, periodic_update, resolve, update_bangs};
use reqwest::Client;
use serde::Deserialize;
use std::fmt::Write;
use std::{env, net::SocketAddr, time::Instant};
use tokio::net::TcpListener;
use tracing::{Level, debug, error, info};

#[derive(Debug, Deserialize)]
struct SearchParams {
    #[serde(rename = "q")]
    query: Option<String>,
}

/// Handler function that extracts the `q` parameter and redirects accordingly
async fn handler(
    Query(params): Query<SearchParams>,
    State(app_state): State<AppState>,
) -> Redirect {
    params.query.map_or_else(
        || Redirect::to("/bangs"),
        |query| {
            let start = Instant::now();
            let redirect_url = resolve(&app_state.get_config(), &query);
            debug!("Request completed in {:?}", start.elapsed());
            info!("Redirecting '{}' to '{}'.", query, redirect_url);
            Redirect::to(&redirect_url)
        },
    )
}

async fn list_bangs(State(app_state): State<AppState>) -> Html<String> {
    let pkg_name = env!("CARGO_PKG_NAME").to_title_case();
    let mut html = String::from(
        "<style>:root { background: #181818; color: #ffffff; font-family: monospace; } table { border-collapse: collapse; width: 100vw; } table th { text-align: left; padding: 1rem 0; font-size: 1.25rem; width: 100vw; } table tr { border-bottom: #ffffff10 solid 2px; } table tr:nth-child(2n) { background: #161616; } table tr:nth-child(2n+1) { background: #181818; }</style><html>",
    );
    html += format!(r#"<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"><link rel="search" type="application/opensearchdescription+xml" title="{pkg_name}" href="/opensearch.xml"/><title>Bang Commands</title></head><body><h1>Bang Commands</h1>"#).as_str();

    if let Some(bangs) = &app_state.get_config().bangs {
        html.push_str("<h2>Configured Bangs</h2><table><th>Abbr.</th><th>Trigger</th><th>URL</th>");
        for bang in bangs {
            write!(
                html,
                "<tr><td><strong>{:?}</strong></td><td>{}</td><td>{}</td></tr>",
                bang.short_name, bang.trigger, bang.url_template
            )
            .expect("Failed to write to HTML string");
        }
        html.push_str("</table>");
    }

    html.push_str("<h2>Active Bangs</h2><table><th>Trigger</th><th>URL</th>");
    for (trigger, url_template) in BANG_CACHE.read().iter() {
        write!(
            html,
            "<tr><td><strong>{trigger}</strong></td><td>{url_template}</td></tr>"
        )
        .expect("Failed to write to HTML string");
    }
    html.push_str("</ul></body></html>");
    Html(html)
}

async fn opensearch(State(app_state): State<AppState>) -> impl IntoResponse {
    let pkg_name = env!("CARGO_PKG_NAME");
    let pkg_description = env!("CARGO_PKG_DESCRIPTION");
    let app_config = app_state.get_config();
    let opensearch_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription
  xmlns="http://a9.com/-/spec/opensearch/1.1/"
  xmlns:moz="http://www.mozilla.org/2006/browser/search/">
  <ShortName>{}</ShortName>
  <Description>{}</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <Url type="text/html" method="GET" template="http://{}:{}/?q={{searchTerms}}" />
  <Url type="application/x-suggestions+json" method="GET" template="http://{}:{}/suggest?q={{searchTerms}}" />
</OpenSearchDescription>"#,
        pkg_name.to_title_case(),
        pkg_description,
        app_config.ip,
        app_config.port,
        app_config.ip,
        app_config.port
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/opensearchdescription+xml"),
    );
    (StatusCode::OK, headers, opensearch_xml)
}

async fn suggestions_proxy(
    Query(params): Query<SearchParams>,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    if let Some(query) = params.query {
        let suggest_api_url = app_state
            .get_config()
            .search_suggestions
            .replace("{}", &query);

        match Client::new().get(&suggest_api_url).send().await {
            Ok(response) => {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    return (StatusCode::OK, headers, Json(json));
                }
            }
            Err(e) => {
                error!("Failed to fetch suggestions from Brave API: {}", e);
            }
        }
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        headers,
        Json(serde_json::json!([])),
    )
}

// endpoint to add a new bang to the config file
async fn add_bang(
    Query(params): Query<redirector::bang::Bang>,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    let mut config = app_state.config.write();
    if let Some(bangs) = &mut config.bangs {
        append_file_config(params.clone());
        bangs.push(params.clone());
        if let Some(mut cache) = BANG_CACHE.try_write() {
            cache.insert(params.trigger, params.url_template);
        }
        return (
            StatusCode::OK,
            headers,
            Json(serde_json::json!({ "status": "success" })),
        );
    }
    drop(config);

    (
        StatusCode::BAD_REQUEST,
        headers,
        Json(serde_json::json!({ "status": "failed" })),
    )
}

#[tokio::main]
async fn main() {
    let cli_config = Cli::parse();

    let log_level = match &cli_config.command {
        Some(SubCommand::Serve { .. }) | None => Level::DEBUG,
        _ => Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_writer(std::io::stderr)
        .init();

    let file_config = get_file_config();

    let app_config = file_config
        .unwrap_or_default()
        .merge(cli_config.clone().into());

    let app_state = AppState::new(app_config.clone());

    match cli_config.command {
        Some(SubCommand::Serve { .. }) | None => {
            tokio::spawn(periodic_update(app_config.clone()));

            let app = Router::new()
                .route("/", get(handler))
                .route("/bangs", get(list_bangs))
                .route("/opensearch.xml", get(opensearch))
                .route("/suggest", get(suggestions_proxy))
                .route("/add_bang", post(add_bang))
                .with_state(app_state);
            let addr = SocketAddr::new(app_config.ip, app_config.port);
            let listener = match TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    error!("Failed to bind to address '{}': {}", addr, e);
                    return;
                }
            };
            info!("Server running on '{}'", addr);
            axum::serve(listener, app).await.unwrap();
        }
        Some(SubCommand::Resolve { query }) => {
            if let Err(e) = update_bangs(&app_config).await {
                error!("Failed to update bang commands: {}", e);
            }
            println!("{}", resolve(&app_config, &query));
        }
        Some(Completions { shell }) => {
            generate(
                shell,
                &mut Cli::command(),
                env!("CARGO_PKG_NAME"),
                &mut std::io::stdout(),
            );
        }
    }
}
