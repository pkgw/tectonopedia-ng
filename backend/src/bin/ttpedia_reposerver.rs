//! The Automerge repository server that is our ultimate document storage
//! backend.
//!
//! Implementation originally cribbed from the Samod JS compatibility test
//! files.

use anyhow::Result;
use axum::Json;
use clap::Parser;
use futures::lock::Mutex;
use samod::{PeerId, Repo, storage::TokioFilesystemStorage};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    data_root: PathBuf,
}

impl Args {
    async fn exec(self) -> Result<()> {
        let builder = Repo::build_tokio();
        let storage = TokioFilesystemStorage::new(self.data_root);
        let builder = builder.with_storage(storage);
        let builder = builder.with_peer_id(PeerId::from_string("ttpedia".to_owned()));
        let samod = builder.load().await;

        let running_connections = Arc::new(Mutex::new(Vec::new()));

        let app = axum::Router::new()
            .route(
                "/ttpapi1/repo/submit",
                axum::routing::post(post_submit_handler),
            )
            .route("/ttpapi1/repo/sync", axum::routing::get(websocket_handler))
            .layer(TraceLayer::new_for_http())
            .with_state((samod.clone(), running_connections.clone()));

        // NB hardcoded testing port
        let listener = TcpListener::bind("0.0.0.0:29180")
            .await
            .expect("unable to bind socket");

        let server = axum::serve(listener, app).into_future();
        println!("listening on: ws://127.0.0.1:29180/");
        println!("peer ID is: {}", samod.peer_id());

        tokio::spawn(server).await??;
        Ok(())
    }
}

#[allow(clippy::type_complexity)]
async fn websocket_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    axum::extract::State((handle, running_connections)): axum::extract::State<(
        Repo,
        Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    )>,
) -> axum::response::Response {
    ws.on_upgrade(|socket| handle_socket(socket, handle, running_connections))
}

async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    repo: Repo,
    running_connections: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
) {
    eprintln!("Accepting websocket connection");
    let driver = repo.accept_axum(socket);
    let handle = tokio::spawn(async {
        let finished = driver.await;
        eprintln!("websocket sync server connection finished: {finished:?}");
    });
    running_connections.lock().await.push(handle);
}

#[derive(Deserialize)]
struct PostSubmitRequest {}

#[derive(Serialize)]
struct PostSubmitResponse {
    status: String,
}

async fn post_submit_handler(Json(_payload): Json<PostSubmitRequest>) -> Json<PostSubmitResponse> {
    Json(PostSubmitResponse {
        status: "ok".to_owned(),
    })
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    //tracing_subscriber::fmt::init();
    //console_subscriber::init();

    if let Err(err) = args.exec().await {
        eprintln!("fatal error: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("caused by: {}", cause));
        std::process::exit(1);
    }
}
