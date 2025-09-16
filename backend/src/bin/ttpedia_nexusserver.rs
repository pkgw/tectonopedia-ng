//! The "nexus" server that is the central gathering point for pedia-wide
//! data.

use anyhow::Result;
use axum::{
    Json,
    http::{HeaderValue, Method, header},
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use ttpedia_backend::{NexusPostPass1Request, NexusPostPass1Response};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    data_root: PathBuf,
}

impl Args {
    async fn exec(self) -> Result<()> {
        let allowed_origin = std::env::var("TTPEDIA_NEXUS_ALLOWED_ORIGIN")?;
        let allowed_origin = allowed_origin.parse::<HeaderValue>()?;

        let state = NexusState {};

        let app = axum::Router::new()
            .route(
                "/ttpapi1/nexus/pass1",
                axum::routing::post(post_pass1_handler),
            )
            .layer(
                CorsLayer::new()
                    .allow_origin(allowed_origin)
                    .allow_methods([Method::GET, Method::POST])
                    .allow_headers([header::CONTENT_TYPE]),
            )
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        // NB hardcoded testing port
        let listener = TcpListener::bind("0.0.0.0:29280")
            .await
            .expect("unable to bind socket");

        let server = axum::serve(listener, app).into_future();
        println!("listening on: http://127.0.0.1:29280/");
        tokio::spawn(server).await??;
        Ok(())
    }
}

#[derive(Clone)]
struct NexusState {}

/// `POST /pass1`: invoked by a TeX compiler worker after its first compilation
/// pass. We process the set of assets required by this build, and return
/// information to the worker to allow it to perform the second pass.
async fn post_pass1_handler(
    axum::extract::State(_state): axum::extract::State<NexusState>,
    Json(_req): Json<NexusPostPass1Request>,
) -> Json<NexusPostPass1Response> {
    Json(NexusPostPass1Response {
        status: "ok".to_owned(),
    })
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    //use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    //tracing_subscriber::registry()
    //    .with(
    //        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
    //            format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
    //        }),
    //    )
    //    .with(tracing_subscriber::fmt::layer())
    //    .init();

    if let Err(err) = args.exec().await {
        eprintln!("fatal error: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("caused by: {}", cause));
        std::process::exit(1);
    }
}
