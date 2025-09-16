//! Miscellaneous utilities for ttpedia.

use anyhow::Result;
use automerge::{Automerge, ObjType, ROOT, transaction::Transactable};
use clap::Parser;
use minio::s3::types::S3Api;
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

    /// Create a bucket in a bucket storage service.
    MakeBucket(MakeBucketCommand),
}

impl Subcommands {
    async fn exec(self) -> Result<()> {
        match self {
            Subcommands::Import(a) => a.exec().await,
            Subcommands::MakeBucket(a) => a.exec().await,
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

#[derive(Parser, Debug)]
#[command()]
struct MakeBucketCommand {
    #[arg(long)]
    public: bool,

    #[arg(long)]
    versioning: bool,

    #[arg()]
    url: String,

    #[arg()]
    bucket: String,
}

impl MakeBucketCommand {
    async fn exec(self) -> Result<()> {
        let bucket_username = std::env::var("TTPEDIA_BUCKET_USERNAME")?;
        let bucket_password = std::env::var("TTPEDIA_BUCKET_PASSWORD")?;

        let base_url: minio::s3::http::BaseUrl = self.url.parse()?;
        let provider =
            minio::s3::creds::StaticProvider::new(&bucket_username, &bucket_password, None);
        let client = minio::s3::client::ClientBuilder::new(base_url)
            .provider(Some(Box::new(provider)))
            .app_info(Some(("ttpedia-tool".to_owned(), "0".to_owned())))
            .build()?;

        let resp = client.create_bucket(&self.bucket).send().await?;
        println!("Made bucket `{}` in region `{}`", resp.bucket, resp.region);

        if self.versioning {
            let resp = client
                .put_bucket_versioning(&self.bucket)
                .versioning_status(minio::s3::builders::VersioningStatus::Enabled)
                .send()
                .await?;
            println!("Enabled versioning on bucket `{}`", resp.bucket);
        }

        if self.public {
            let resp = client
                .put_bucket_policy(&self.bucket)
                .config(format!(
                    r#"{{
                        "Version": "2012-10-17",
                        "Statement": [
                            {{
                                "Effect": "Allow",
                                "Principal": {{
                                    "AWS": ["*"]
                                }},
                                "Action": ["s3:GetObject"],
                                "Resource": ["arn:aws:s3:::{}/*"]
                            }}
                        ]
                    }}"#,
                    self.bucket
                ))
                .send()
                .await?;
            println!("Enabled readonly access on bucket `{}`", resp.bucket);
        }

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
