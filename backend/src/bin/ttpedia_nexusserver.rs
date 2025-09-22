//! The "nexus" server that is the central gathering point for pedia-wide
//! data.

use anyhow::Result;
use axum::{
    Json,
    extract::Path,
    http::{HeaderValue, Method, header},
    response::Redirect,
};
use clap::Parser;
use futures::lock::Mutex;
use lmdb::{Environment, EnvironmentFlags};
use std::{
    fmt::Write,
    io::{BufRead, BufReader, Cursor},
    path::PathBuf,
    sync::Arc,
};
use tectonic_engine_spx2html::AssetSpecification;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use ttpedia_backend::{
    NexusPostAssetsUploadedRequest, NexusPostAssetsUploadedResponse, NexusPostPass1Request,
    NexusPostPass1Response,
    metadata::{IndexRefFlag, Metadatum},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    data_root: PathBuf,
}

impl Args {
    async fn exec(self) -> Result<()> {
        let allowed_origin = std::env::var("TTPEDIA_NEXUS_ALLOWED_ORIGIN")?;
        let allowed_origin = allowed_origin.parse::<HeaderValue>()?;

        let public_data_url = std::env::var("TTPEDIA_PUBLIC_DATA_URL")?;

        let cur_assets = AssetSpecification::default();

        // XXX: recover assets bucket key from persistent storage,
        // and get assets.json from the bucket. Builders will need to
        // upload their assets.json! Or we can save it locally.
        //
        //let mut assets_save_path = self.data_root.clone();
        //assets_save_path.push("assets.json");
        //if let Ok(saved) = std::fs::File::open(&assets_save_path) {
        //    cur_assets.add_from_saved(saved)?;
        //}

        let state = NexusState {
            assets: Arc::new(Mutex::new(AssetState {
                cur_assets,
                cur_seqnum: 0,
                cur_bucket_key: "FIXME-get-from-storage".to_owned(),
                next_proposed_seqnum: 1,
            })),
            public_data_url,
        };

        let app = axum::Router::new()
            .route(
                "/ttpapi1/nexus/pass1",
                axum::routing::post(post_pass1_handler),
            )
            .route(
                "/ttpapi1/nexus/assets_uploaded",
                axum::routing::post(post_assets_uploaded_handler),
            )
            .route(
                "/ttpapi1/nexus/asset/{key}",
                axum::routing::get(get_asset_handler),
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

struct AssetState {
    cur_assets: AssetSpecification,
    cur_seqnum: usize,
    cur_bucket_key: String,
    next_proposed_seqnum: usize,
}

#[derive(Clone)]
struct NexusState {
    assets: Arc<Mutex<AssetState>>,
    public_data_url: String,
}

/// `POST /pass1`: invoked by a TeX compiler worker after its first compilation
/// pass. We process the set of assets required by this build, and return
/// information to the worker to allow it to perform the second pass.
async fn post_pass1_handler(
    axum::extract::State(state): axum::extract::State<NexusState>,
    Json(req): Json<NexusPostPass1Request>,
) -> Json<NexusPostPass1Response> {
    // Handle the assets

    let mut assets = state.assets.lock().await;

    let pass1_assets = Cursor::new(req.assets_json.as_bytes());
    assets
        .cur_assets
        .add_from_saved(pass1_assets)
        .expect("parse and no conflicts");

    let mut pass2_assets: Vec<u8> = Default::default();
    assets
        .cur_assets
        .save(&mut pass2_assets)
        .expect("save to bytes OK");

    let pass2_assets = String::from_utf8(pass2_assets).expect("saved is string");
    let mut preserve_assets = None;

    // HACK: tell every build to update assets. We should only do this if they
    // actually need updating.
    if true {
        preserve_assets = Some(assets.next_proposed_seqnum);
        assets.next_proposed_seqnum += 1;
    }

    // Handle cross-reference requests

    let pass1_xrefs = Cursor::new(req.pedia_txt.as_bytes());
    let meta_buf = BufReader::new(pass1_xrefs);
    let mut rrtex = String::new();

    for line in meta_buf.lines() {
        // For this pass, we can ignore everything besides references.

        let line = line.expect("readline");

        match Metadatum::parse(&line).expect("parse metaline") {
            Metadatum::IndexRef {
                index,
                entry,
                flags,
            } => {
                if (flags & IndexRefFlag::NeedsLoc as u8) != 0 {
                    writeln!(
                        rrtex,
                        r"\expandafter\def\csname pedia resolve**{}**{}**loc\endcsname{{{}}}",
                        index, entry, "LOCREF",
                    )
                    .unwrap();
                }

                if (flags & IndexRefFlag::NeedsText as u8) != 0 {
                    writeln!(
                        rrtex,
                        r"\expandafter\def\csname pedia resolve**{}**{}**text tex\endcsname{{{}}}",
                        index, entry, entry,
                    )
                    .unwrap();
                    writeln!(
                        rrtex,
                        r"\expandafter\def\csname pedia resolve**{}**{}**text plain\endcsname{{{}}}",
                        index, entry, entry,
                    )
                    .unwrap();
                }
            }

            _ => {}
        }
    }

    // All done!

    Json(NexusPostPass1Response {
        status: "ok".to_owned(),
        assets_json: pass2_assets,
        resolved_reference_tex: rrtex,
        preserve_assets,
    })
}

/// `POST /assets_uploaded`: invoked by a TeX compiler worker after it has
/// uploaded the assets that it generated to the shared bucket, if it was
/// instructed to do so.
async fn post_assets_uploaded_handler(
    axum::extract::State(state): axum::extract::State<NexusState>,
    Json(req): Json<NexusPostAssetsUploadedRequest>,
) -> Json<NexusPostAssetsUploadedResponse> {
    // We might tell a several builds to upload assets quasi-simultaneously, and
    // we can't predict the order in which responses will come back. If an early
    // one comes back late, it's been superseded, and we should just ignore it.

    let mut assets = state.assets.lock().await;

    if req.seq_num > assets.cur_seqnum {
        assets.cur_bucket_key = req.bucket_key;
        assets.cur_seqnum = req.seq_num;
        // TODO: serialize bucket key!!!!
    }

    Json(NexusPostAssetsUploadedResponse {})
}

/// `GET /asset/{key}`: get a shared asset.
async fn get_asset_handler(
    axum::extract::State(state): axum::extract::State<NexusState>,
    Path(key): Path<String>,
) -> Redirect {
    let assets = state.assets.lock().await;

    // TODO/FIXME? Stream out of the bucket rather than redirecting?
    Redirect::temporary(&format!(
        "{}/sharedassets/{}/{}",
        state.public_data_url, assets.cur_bucket_key, key
    ))
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
