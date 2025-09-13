//! The compiler worker that compiles TeX!
//!
//! See
//! <https://docs.rs/faktory/0.13.1/faktory/struct.WorkerBuilder.html#method.with_graceful_shutdown>
//! for example of how to add a graceful shutdown mode here.

use anyhow::Result;
use clap::Parser;
use faktory::{Job, Worker};

const NUM_WORKERS: usize = 2;

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

fn do_compile(job: Job) -> Result<(), faktory::Error> {
    let doc_id = job.args()[0].as_str().unwrap();
    println!("got job {:?} for doc {doc_id}", job.id());
    let content = job.args()[1].as_str().unwrap();
    println!("content:\n{content}");
    Ok(())
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
