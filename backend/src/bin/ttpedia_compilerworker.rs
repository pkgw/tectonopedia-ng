//! The compiler worker that compiles TeX!
//!
//! See
//! <https://docs.rs/faktory/0.13.1/faktory/struct.WorkerBuilder.html#method.with_graceful_shutdown>
//! for example of how to add a graceful shutdown mode here.

use anyhow::Result;
use clap::Parser;
use faktory::{Job, Worker};
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use tectonic::{
    config::PersistentConfig,
    driver::{OutputFormat, PassSetting, ProcessingSessionBuilder},
    status::termcolor::TermcolorStatusBackend,
    unstable_opts::UnstableOptions,
};
use tectonic_bridge_core::{SecuritySettings, SecurityStance};
use tectonic_status_base::ChatterLevel;

use ttpedia_backend::{NexusPostPass1Request, NexusPostPass1Response};

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
    nexus_url: String,
}

impl Config {
    fn new(args: Args) -> Result<Self> {
        let nexus_url = std::env::var("TTPEDIA_NEXUS_URL")?;

        Ok(Config {
            defs_dir: args.defs_dir,
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
            .register_blocking_fn("compile", do_compile)
            .connect()
            .await
            .unwrap();

        let outcome = worker.run(&["default"]).await?;
        println!("decided to exit: {outcome:?}");
        Ok(())
    }
}

/// Compile a TeX document in the Tectonopedia framework.
fn do_compile(job: Job) -> Result<(), faktory::Error> {
    let config = GLOBAL_CONFIG_HACK.get().unwrap();
    let mut state = CompileState::new(config, &job);
    state.pass1()?;
    Ok(())
}

/// `doc_id` and `content` are references into the Job objet so it's easiest to
/// have all of that be borrowed.
#[derive(Debug)]
struct CompileState<'a> {
    config: &'a Config,
    job: &'a Job,
    doc_id: &'a str,
    content: &'a str,
}

impl<'a> CompileState<'a> {
    fn new(config: &'a Config, job: &'a Job) -> Self {
        let doc_id = job.args()[0].as_str().unwrap();
        let content = job.args()[1].as_str().unwrap();
        CompileState {
            config,
            job,
            doc_id,
            content,
        }
    }
}

impl<'a> CompileState<'a> {
    /// First compilation pass.
    fn pass1(&mut self) -> Result<(), faktory::Error> {
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
            self.content,
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

        let req = NexusPostPass1Request {
            doc_id: self.doc_id.to_owned(),
            job_id: self.job.id().to_string(),
            assets_json: assets,
            pedia_txt: links,
        };

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(format!("{}/pass1", self.config.nexus_url))
            .json(&req)
            .send()
            .expect("HTTP pass1 to nexus didnt send")
            .error_for_status()
            .expect("HTTP pass1 to nexus failed");
        let payload = resp
            .json::<NexusPostPass1Response>()
            .expect("HTTP pass1 resp json");
        println!("response from nexus: {payload:?}");
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
