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
use lmdb::{Environment, EnvironmentFlags, Transaction};
use std::{
    collections::HashMap,
    fmt::Write,
    io::{BufRead, BufReader, Cursor},
    path::PathBuf,
    sync::Arc,
};
use tectonic_engine_spx2html::AssetSpecification;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use ttpedia_backend::{
    NexusGetEntryResponse, NexusPostAssetsUploadedRequest, NexusPostAssetsUploadedResponse,
    NexusPostPass1Request, NexusPostPass1Response,
    metadata::{IndexRefFlag, Metadatum},
};

const DB_FORMAT_SERIAL: usize = 0;

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

        let mut db_path = self.data_root.clone();
        db_path.push(format!("nexus_state_v{DB_FORMAT_SERIAL}.lmdb"));
        let env = Environment::new()
            .set_flags(EnvironmentFlags::NO_SUB_DIR)
            .set_max_dbs(4)
            .set_map_size(268_435_456)
            .open(&db_path)?;

        let state = NexusState {
            assets: Arc::new(Mutex::new(AssetState {
                cur_assets,
                cur_seqnum: 0,
                cur_bucket_key: "FIXME-get-from-storage".to_owned(),
                next_proposed_seqnum: 1,
            })),
            db: Arc::new(env),
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
            .route(
                "/ttpapi1/nexus/entry/{name}",
                axum::routing::get(get_entry_handler),
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
    db: Arc<Environment>,
    public_data_url: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct IndexKey {
    pub index: String,
    pub entry: String,
}

impl IndexKey {
    fn new<S1: ToString, S2: ToString>(index: S1, entry: S2) -> Self {
        IndexKey {
            index: index.to_string(),
            entry: entry.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
struct IndexValue {
    pub entry: Option<String>,
    pub fragment: Option<String>,
    pub atplain: Option<String>,
    pub tex: Option<String>,
}

const INDEX_DEF_MARKER: u8 = 0x80;
const MISSING_REF: &[u8] = &[0, 0];

fn maybe_slice_to_str_or_default<'a>(b: Option<&'a [u8]>, default: &'a str) -> &'a str {
    let Some(b) = b else {
        return default;
    };

    if b.is_empty() {
        return default;
    }

    match str::from_utf8(b) {
        Ok(s) => s,
        Err(_) => default,
    }
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

    // Handle cross-references
    //
    // TBD: do we want to handle definitions after pass 2? Maybe? But if we do
    // them here, we can avoid having to re-send the `pedia.txt` data after that
    // pass completes ...

    let pedia_txt = req.pedia_txt;
    let dbenv = state.db.clone();

    let rrtex = tokio::task::spawn_blocking(move || -> Result<String> {
        let db = dbenv
            .create_db(Some("index"), Default::default())
            .expect("open db");
        let mut txn = dbenv.begin_rw_txn().expect("rw txn");

        let mut current_entry = "".to_owned();
        let pass1_xrefs = Cursor::new(pedia_txt.as_bytes());
        let meta_buf = BufReader::new(pass1_xrefs);
        let mut rrtex = String::new();
        let mut defs: HashMap<IndexKey, IndexValue> = Default::default();

        for line in meta_buf.lines() {
            let line = line.expect("readline");

            match Metadatum::parse(&line).expect("parse metaline") {
                Metadatum::IndexRef {
                    index,
                    entry,
                    flags,
                } => {
                    let mut bkey = vec![INDEX_DEF_MARKER];
                    bkey.extend_from_slice(index.as_bytes());
                    bkey.push(0);
                    bkey.extend_from_slice(entry.as_bytes());

                    let bvalue = txn.get(db, &bkey).unwrap_or(MISSING_REF);
                    let mut fields = bvalue.split(|b| *b == 0);
                    let entry_slice = fields.next();
                    let fragment_slice = fields.next();

                    if (flags & IndexRefFlag::NeedsLoc as u8) != 0 {
                        let entry_text = maybe_slice_to_str_or_default(entry_slice, "ENTRYREF");
                        let fragment_text = maybe_slice_to_str_or_default(fragment_slice, "");
                        writeln!(
                            rrtex,
                            r"\expandafter\def\csname pedia resolve**{}**{}**loc\endcsname{{{}{}}}",
                            index, entry, entry_text, fragment_text,
                        )
                        .unwrap();
                    }

                    let atplain_slice = fields.next();
                    let tex_slice = fields.next();

                    if (flags & IndexRefFlag::NeedsText as u8) != 0 {
                        let atplain_text = maybe_slice_to_str_or_default(atplain_slice, entry);
                        let tex_text = maybe_slice_to_str_or_default(tex_slice, entry);

                        writeln!(
                            rrtex,
                            r"\expandafter\def\csname pedia resolve**{}**{}**text tex\endcsname{{{}}}",
                            index, entry, tex_text,
                        )
                        .unwrap();
                        writeln!(
                            rrtex,
                            r"\expandafter\def\csname pedia resolve**{}**{}**text plain\endcsname{{{}}}",
                            index, entry, atplain_text,
                        )
                        .unwrap();
                    }
                }

                Metadatum::IndexDef {
                    index,
                    entry,
                    fragment,
                } => {
                    let val = defs.entry(IndexKey::new(index, entry)).or_default();
                    val.entry = Some(current_entry.clone());
                    val.fragment = Some(fragment.to_string());
                }

                Metadatum::IndexText {
                    index,
                    entry,
                    tex,
                    atplain,
                } => {
                    let val = defs.entry(IndexKey::new(index, entry)).or_default();
                    val.atplain = Some(atplain.to_string());
                    val.tex = Some(tex.to_string());
                }

                Metadatum::Output(o) => {
                    current_entry = o.strip_prefix("entry-").and_then(|s| s.strip_suffix(".html")).unwrap_or_default().to_owned();
                }
            }
        }

        // Record new index definitions in the database

        for (key, value) in defs.drain() {
            let mut bkey = vec![INDEX_DEF_MARKER];
            bkey.append(&mut key.index.into_bytes());
            bkey.push(0);
            bkey.append(&mut key.entry.into_bytes());

            let mut bvalue = value.entry.unwrap_or_default().into_bytes();
            bvalue.push(0);
            bvalue.append(&mut value.fragment.unwrap_or_default().into_bytes());
            bvalue.push(0);
            bvalue.append(&mut value.atplain.unwrap_or_default().into_bytes());
            bvalue.push(0);
            bvalue.append(&mut value.tex.unwrap_or_default().into_bytes());

            txn.put(db, &bkey, &bvalue, Default::default())
                .expect("put");
        }

        txn.commit().expect("commit txn");

        Ok(rrtex)
    }).await.expect("join").expect("handled refs");

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

/// `GET /entry/{name}`: fetch needed info to render an entry page
async fn get_entry_handler(
    //axum::extract::State(state): axum::extract::State<NexusState>,
    Path(name): Path<String>,
) -> Json<NexusGetEntryResponse> {
    println!("FIXME: fake getentry mapping!");

    let (doc_id, output_name, title) = match name.as_ref() {
        "dump" => ("gxhZkppeZEXBb7LXnwvHWEuavAd", "dump.html", r"\dump"),
        "index" => ("tsquNfquQC6eLNYP7ZmgmNkbwXP", "index.html", "Index"),
        "end" => ("25spacqQwZqMUBMkrCJB1ot1EmGq", "end.html", r"\end"),
        "message" => ("3huRDC2cWvQhEeFxezP58NWxnMk9", "message.html", r"\message"),
        "why-tex" => ("3XuSpKARAcsShsAFTZBJKBxwjRsz", "why-tex.html", "Why TeX?"),
        _ => ("NOT-FOUND", "notfound.html", "Not Found"),
    };

    let (doc_id, output_name, title) =
        (doc_id.to_owned(), output_name.to_owned(), title.to_owned());

    Json(NexusGetEntryResponse {
        doc_id,
        output_name,
        title,
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
