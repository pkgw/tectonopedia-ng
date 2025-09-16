//! Code shared between the various Tectonopedia Rust servers

use serde::{Deserialize, Serialize};

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
}
