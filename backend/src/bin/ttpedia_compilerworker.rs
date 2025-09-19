//! The compiler worker that compiles TeX!
//!
//! See
//! <https://docs.rs/faktory/0.13.1/faktory/struct.WorkerBuilder.html#method.with_graceful_shutdown>
//! for example of how to add a graceful shutdown mode here.

use anyhow::Result;
use clap::Parser;
use faktory::{Job, Worker};
use once_cell::sync::OnceCell;
use std::{io::Cursor, path::PathBuf};
use tectonic::{
    config::PersistentConfig,
    driver::{OutputFormat, PassSetting, ProcessingSessionBuilder},
    status::termcolor::TermcolorStatusBackend,
    unstable_opts::UnstableOptions,
};
use tectonic_bridge_core::{SecuritySettings, SecurityStance};
use tectonic_engine_spx2html::AssetSpecification;
use tectonic_status_base::ChatterLevel;
use tempfile::TempDir;

use ttpedia_backend::{
    NexusPostAssetsUploadedRequest, NexusPostAssetsUploadedResponse, NexusPostPass1Request,
    NexusPostPass1Response,
};

const NUM_WORKERS: usize = 1; // with the global Tectonic mutex, we're stuck with this
const DEBUG: bool = false;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    defs_dir: PathBuf,
}

#[derive(Debug)]
struct Config {
    defs_dir: PathBuf,
    bucket_url: String,
    bucket_username: String,
    bucket_password: String,
    nexus_url: String,
}

impl Config {
    fn new(args: Args) -> Result<Self> {
        let bucket_url = std::env::var("TTPEDIA_BUCKET_URL")?;
        let bucket_username = std::env::var("TTPEDIA_BUCKET_USERNAME")?;
        let bucket_password = std::env::var("TTPEDIA_BUCKET_PASSWORD")?;
        let nexus_url = std::env::var("TTPEDIA_NEXUS_URL")?;

        Ok(Config {
            defs_dir: args.defs_dir,
            bucket_url,
            bucket_username,
            bucket_password,
            nexus_url,
        })
    }
}

/// The do_compile() function must be static according to faktory-rs's typing,
/// so I think we need a construct like this to allow it to access the runtime
/// args. There's almost surely a better way to do this.
static GLOBAL_CONFIG_HACK: OnceCell<Config> = OnceCell::new();

impl Args {
    async fn exec(self) -> Result<()> {
        let config = Config::new(self)?;
        GLOBAL_CONFIG_HACK.get_or_init(|| config);

        let mut worker = Worker::builder()
            .workers(NUM_WORKERS)
            .register_fn("compile", do_compile)
            .connect()
            .await
            .unwrap();

        let outcome = worker.run(&["default"]).await?;
        println!("decided to exit: {outcome:?}");
        Ok(())
    }
}

/// Compile a TeX document in the Tectonopedia framework.
///
/// FIXME: return type needs to be a faktory Error? If so we need to add some
/// magic to be able to use boxed errors internally because nah.
async fn do_compile(job: Job) -> Result<(), faktory::Error> {
    let config = GLOBAL_CONFIG_HACK.get().unwrap();
    let mut state = CompileState::new(config, job);

    // Compilation pass 1 - blocking
    let (req, mut state) = tokio::task::spawn_blocking(move || -> Result<_, faktory::Error> {
        let req = state.pass1()?;
        Ok((req, state))
    })
    .await
    .expect("join")?;

    // Submit to nexus and process results
    let resp = state.nexus1(req).await?;
    let preserve_assets = resp.preserve_assets;

    // Compilation pass 2.
    let (out_dir, state) = tokio::task::spawn_blocking(move || -> Result<_, faktory::Error> {
        let out_dir = state.pass2(resp)?;
        Ok((out_dir, state))
    })
    .await
    .expect("join")?;

    // upload to bucket
    state.upload_to_bucket(out_dir, preserve_assets).await?;

    Ok(())
}

/// `doc_id` and `content` are references into the Job objet so it's easiest to
/// have all of that be borrowed.
#[derive(Debug)]
struct CompileState<'a> {
    config: &'a Config,
    job: Job,
}

impl<'a> CompileState<'a> {
    fn new(config: &'a Config, job: Job) -> Self {
        CompileState { config, job }
    }

    fn doc_id(&self) -> &str {
        self.job.args()[0].as_str().unwrap()
    }

    fn content(&self) -> &str {
        self.job.args()[1].as_str().unwrap()
    }
}

impl<'a> CompileState<'a> {
    /// First compilation pass.
    fn pass1(&mut self) -> Result<NexusPostPass1Request, faktory::Error> {
        let mut status = TermcolorStatusBackend::new(ChatterLevel::default());
        let config: PersistentConfig = PersistentConfig::open(false).expect("config");
        let security = SecuritySettings::new(SecurityStance::MaybeAllowInsecures);

        let mut cls = self.config.defs_dir.clone();
        cls.push("cls");
        let unstables = UnstableOptions {
            extra_search_paths: vec![cls],
            ..UnstableOptions::default()
        };

        let input = format!(
            "\\newif\\ifpassone \
            \\passonetrue \
            \\input{{preamble}}
            {}
            \\input{{postamble}}\n",
            self.content(),
        );

        let mut sess = ProcessingSessionBuilder::new_with_security(security);
        sess.primary_input_buffer(input.as_bytes())
            .tex_input_name("texput")
            .build_date(std::time::SystemTime::now())
            .bundle(config.default_bundle(false).expect("defaultbundle"))
            .format_name("latex")
            .output_format(OutputFormat::Html)
            .do_not_write_output_files()
            .filesystem_root(&self.config.defs_dir)
            .unstables(unstables)
            .format_cache_path(config.format_cache_path().expect("cachepath"))
            .html_emit_files(false)
            .html_assets_spec_path("assets.json")
            .pass(PassSetting::Default);

        if DEBUG {
            sess.print_stdout(true);
        }

        let mut sess = sess.create(&mut status).expect("create");

        // Print more details in the error case here?
        sess.run(&mut status).expect("run!");

        // Gather the metadata and report them to the Nexus server.

        let mut files = sess.into_file_data();

        let assets = files
            .remove("assets.json")
            .expect("no `assets.json` file output");
        let assets = String::from_utf8(assets.data).expect("`assets.json` not UTF8");

        let links = files
            .remove("pedia.txt")
            .expect("no `pedia.txt` file output");
        let links = String::from_utf8(links.data).expect("`pedia.txt` not UTF8");

        Ok(NexusPostPass1Request {
            doc_id: self.doc_id().to_owned(),
            job_id: self.job.id().to_string(),
            assets_json: assets,
            pedia_txt: links,
        })
    }

    async fn nexus1(
        &mut self,
        req: NexusPostPass1Request,
    ) -> Result<NexusPostPass1Response, faktory::Error> {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/pass1", self.config.nexus_url))
            .json(&req)
            .send()
            .await
            .expect("HTTP pass1 to nexus didnt send")
            .error_for_status()
            .expect("HTTP pass1 to nexus failed");
        let payload = resp
            .json::<NexusPostPass1Response>()
            .await
            .expect("HTTP pass1 resp json");

        Ok(payload)
    }

    /// Second compilation pass.
    ///
    /// Note: need to return the TempDir so as not to delete it!
    fn pass2(&mut self, resp: NexusPostPass1Response) -> Result<TempDir, faktory::Error> {
        let mut status = TermcolorStatusBackend::new(ChatterLevel::default());
        let config: PersistentConfig = PersistentConfig::open(false).expect("config");
        let security = SecuritySettings::new(SecurityStance::MaybeAllowInsecures);

        let mut assets = AssetSpecification::default();
        assets
            .add_from_saved(Cursor::new(resp.assets_json.as_bytes()))
            .expect("add assets");

        let mut cls = self.config.defs_dir.clone();
        cls.push("cls");
        let unstables = UnstableOptions {
            extra_search_paths: vec![cls],
            ..UnstableOptions::default()
        };

        let out_dir = TempDir::new().expect("make tempdir");

        let rrtex = ""; // TODO: TeX of resolved reference info

        let input = format!(
            "\\newif\\ifpassone \
            \\passonefalse \
            \\input{{preamble}}
            {}
            {}
            \\input{{postamble}}\n",
            rrtex,
            self.content(),
        );

        let mut sess = ProcessingSessionBuilder::new_with_security(security);
        sess.primary_input_buffer(input.as_bytes())
            .tex_input_name("texput")
            .build_date(std::time::SystemTime::now())
            .bundle(config.default_bundle(false).expect("defaultbundle"))
            .format_name("latex")
            .output_format(OutputFormat::Html)
            .html_precomputed_assets(assets)
            .filesystem_root(&self.config.defs_dir)
            .unstables(unstables)
            .format_cache_path(config.format_cache_path().expect("cachepath"))
            .output_dir(&out_dir)
            .html_emit_files(true)
            .html_emit_assets(resp.preserve_assets.is_some())
            .pass(PassSetting::Default);

        if DEBUG {
            sess.print_stdout(true);
        }

        let mut sess = sess.create(&mut status).expect("create");

        // Print more details in the error case here?
        sess.run(&mut status).expect("run!");

        // Gather results ...

        println!("pass 2 done");
        let mut files = sess.into_file_data();

        for (fname, finfo) in files.drain() {
            println!("- memfile: {fname}: {}", finfo.data.len());
        }

        Ok(out_dir)
    }

    async fn upload_to_bucket(
        &self,
        out_dir: TempDir,
        preserve_assets: Option<usize>,
    ) -> Result<(), faktory::Error> {
        let base_url: minio::s3::http::BaseUrl = self.config.bucket_url.parse().expect("parse URL");
        let provider = minio::s3::creds::StaticProvider::new(
            &self.config.bucket_username,
            &self.config.bucket_password,
            None,
        );
        let client = minio::s3::client::ClientBuilder::new(base_url)
            .provider(Some(Box::new(provider)))
            .app_info(Some(("compilerworker".to_owned(), "0".to_owned())))
            .build()
            .expect("minio client build");

        let mut dir = tokio::fs::read_dir(&out_dir).await.expect("readdir");
        let mut assets = Vec::new();
        let mut htmls = Vec::new();

        // Scan the output dir for stuff we might need to upload.

        while let Some(entry) = dir.next_entry().await.expect("readdirent") {
            let os_name = entry.file_name();
            let Some(str_name) = os_name.to_str() else {
                continue;
            };

            if preserve_assets.is_some() {
                if str_name.ends_with(".otf") || str_name.ends_with(".css") {
                    assets.push(entry.path());
                    continue;
                }
            }

            if str_name.starts_with("entry-") {
                htmls.push(entry.path());
            }
        }

        // Upload assets if requested.

        for asset_path in assets.drain(..) {
            let asset_filename = asset_path.file_name().unwrap().to_str().unwrap();
            let object = format!("{}/{}", self.job.id().to_string(), asset_filename);

            let content_type = if asset_filename.ends_with(".css") {
                "text/css"
            } else if asset_filename.ends_with(".otf") {
                "font/otf"
            } else {
                "application/octet-stream"
            };

            let content: minio::s3::builders::ObjectContent = asset_path.as_path().into();

            let resp = client
                .put_object_content("ttpedia-sharedassets", object, content)
                .content_type(content_type.to_owned())
                .send()
                .await
                .unwrap();
            println!(
                "  ... uploaded sharedassets object `{}` with ETag `{}`",
                resp.object, resp.etag
            );
        }

        // If that all worked, and we're preserving our assets, notify the nexus server to update
        // its knowledge of the shared assets.

        if let Some(seq_num) = preserve_assets {
            let req = NexusPostAssetsUploadedRequest {
                seq_num,
                bucket_key: self.job.id().to_string(),
            };

            println!("notifying uploaded: {:?}", req);

            let client = reqwest::Client::new();
            let resp = client
                .post(format!("{}/assets_uploaded", self.config.nexus_url))
                .json(&req)
                .send()
                .await
                .expect("HTTP assets-upload to nexus didnt send")
                .error_for_status()
                .expect("HTTP assets-upload to nexus failed");

            // response is vacuous
            resp.json::<NexusPostAssetsUploadedResponse>()
                .await
                .expect("HTTP assets-upload resp json");
        }

        // If the shared assets are sufficiently up-to-date, we can upload the
        // actual HTMLs.

        for html_path in htmls.drain(..) {
            let stem = html_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .strip_prefix("entry-")
                .unwrap();

            let object = format!("{}/{}", self.doc_id(), stem);
            let content: minio::s3::builders::ObjectContent = html_path.as_path().into();

            let resp = client
                .put_object_content("ttpedia-html", object, content)
                .content_type("text/html".to_owned())
                .send()
                .await
                .unwrap();
            println!(
                "  ... uploaded html object `{}` with ETag `{}`",
                resp.object, resp.etag
            );
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(err) = args.exec().await {
        eprintln!("fatal error: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("caused by: {}", cause));
        std::process::exit(1);
    }
}
