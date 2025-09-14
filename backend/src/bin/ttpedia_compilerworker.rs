//! The compiler worker that compiles TeX!
//!
//! See
//! <https://docs.rs/faktory/0.13.1/faktory/struct.WorkerBuilder.html#method.with_graceful_shutdown>
//! for example of how to add a graceful shutdown mode here.

use anyhow::Result;
use clap::Parser;
use faktory::{Job, Worker};
use std::io::{BufRead, BufReader, Cursor};
use tectonic::{
    config::PersistentConfig,
    driver::{OutputFormat, PassSetting, ProcessingSessionBuilder},
    status::termcolor::TermcolorStatusBackend,
    unstable_opts::UnstableOptions,
};
use tectonic_bridge_core::{SecuritySettings, SecurityStance};
use tectonic_status_base::ChatterLevel;

const NUM_WORKERS: usize = 1; // with the global Tectonic mutex, we're stuck with this
const DEBUG: bool = false;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

impl Args {
    async fn exec(self) -> Result<()> {
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
    let mut state: CompileState<'_> = (&job).into();
    state.pass1()?;
    Ok(())
}

/// `doc_id` and `content` are references into the Job objet so it's easiest to
/// have all of that be borrowed.
#[derive(Debug)]
struct CompileState<'a> {
    job: &'a Job,
    doc_id: &'a str,
    content: &'a str,
}

impl<'a> From<&'a Job> for CompileState<'a> {
    fn from(job: &'a Job) -> Self {
        let doc_id = job.args()[0].as_str().unwrap();
        let content = job.args()[1].as_str().unwrap();
        CompileState {
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

        //let root = gtry!(crate::config::get_root());
        //let mut cls = root.clone();
        //cls.push("cls");
        let unstables = UnstableOptions {
            //extra_search_paths: vec![cls],
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
            //.filesystem_root(root)
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

        // Print out the assets info

        let mut files = sess.into_file_data();

        let assets = files
            .remove("assets.json")
            .expect("no `assets.json` file output");
        let assets = BufReader::new(Cursor::new(&assets.data));

        for line in assets.lines() {
            let line = line.expect("error reading line of `assets.json` output");
            println!("pedia:assets {}", line);
        }

        // Print out the `pedia.txt` metadata file

        let assets = files
            .remove("pedia.txt")
            .expect("no `pedia.txt` file output");
        let assets = BufReader::new(Cursor::new(&assets.data));

        for line in assets.lines() {
            let line = line.expect("error reading line of `pedia.txt` output");
            println!("pedia:meta {}", line);
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
