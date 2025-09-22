//! Code shared between the various Tectonopedia Rust servers

use serde::{Deserialize, Serialize};

pub mod metadata;

/// The request to the Nexus server's `POST /pass1` endpoint, which is invoked
/// when a compiler worker has completed a first compilation pass. This provides
/// information about the assets required by the document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NexusPostPass1Request {
    /// The automerge-repo ID of the document being compiled, in its base58check
    /// representation.
    pub doc_id: String,

    /// The ID of the compilation job (originally assigned by Faktory).
    pub job_id: String,

    /// The contents of the `assets.json` file.
    pub assets_json: String,

    /// The contents of the `pedia.txt` file.
    pub pedia_txt: String,
}

/// The response from the Nexus server's `POST /pass1` endpoint. It returns the
/// asset information needed to perform the second pass.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NexusPostPass1Response {
    /// "ok" if success, a brief error message if not.
    pub status: String,

    /// Updated assets data, merging all processed documents.
    pub assets_json: String,

    /// TeX containing resolved cross-reference information
    pub resolved_reference_tex: String,

    /// If provided, the builder should upload its asset files to the bucket,
    /// and follow up with confirmation if/when it succeeds, returning the
    /// sequence number that it's been provided.
    pub preserve_assets: Option<usize>,
}

/// The request to the Nexus server's `POST /assets_uploaded` endpoint, which is
/// invoked when a compiler worker has uploaded its assets to the "shared
/// assets" bucket after being instructed to do so.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NexusPostAssetsUploadedRequest {
    /// The sequence number that was provided to the build worker in the
    /// pass 1 response.
    pub seq_num: usize,

    /// The unique key prefix under which the assets are stored in the bucket.
    /// Should not contain any slashes.
    pub bucket_key: String,
}

/// The response from the Nexus server's `POST /assets_uploaded` endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NexusPostAssetsUploadedResponse {}
