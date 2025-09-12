//! Miscellaneous utilities for ttpedia.

use anyhow::Result;
use automerge::{Automerge, ObjType, ROOT, transaction::Transactable};
use clap::Parser;
use samod::{Repo, storage::TokioFilesystemStorage};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Subcommands,
}

#[derive(Parser, Debug)]
enum Subcommands {
    /// Import a file into an on-disk repo.
    Import(ImportCommand),
}

impl Subcommands {
    async fn exec(self) -> Result<()> {
        match self {
            Subcommands::Import(a) => a.exec().await,
        }
    }
}

#[derive(Parser, Debug)]
#[command()]
struct ImportCommand {
    #[arg()]
    data_root: PathBuf,

    #[arg()]
    input: PathBuf,
}

impl ImportCommand {
    async fn exec(self) -> Result<()> {
        let builder = Repo::build_tokio();
        let storage = TokioFilesystemStorage::new(self.data_root);
        let builder = builder.with_storage(storage);
        let samod = builder.load().await;

        let content = std::fs::read_to_string(&self.input)?;

        // Note: serde-automerge serializes String to immutable strings, not
        // Text, so we have to populate the document manually.
        let mut doc = Automerge::new();
        doc.transact(|txn| -> Result<()> {
            let text = txn.put_object(ROOT, "content", ObjType::Text)?;
            txn.splice_text(&text, 0, 0, &content)?;
            Ok(())
        })
        .map_err(|f| f.error)?;

        let handle = samod.create(doc).await?;
        println!("{}", handle.document_id());
        samod.stop().await;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    //tracing_subscriber::fmt::init();
    //console_subscriber::init();

    if let Err(err) = args.cmd.exec().await {
        eprintln!("fatal error: {}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| eprintln!("caused by: {}", cause));
        std::process::exit(1);
    }
}
