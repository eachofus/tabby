// === IMPORTS ===
use clap::crate_version;

// === CONSTANTS ===
/// Client identification string used for HTTP requests
/// 
/// Constructed from the repository URL and current crate version.
/// Format: "{repository_url}/releases/tag/{version}"
pub const CLIENT_ID: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/releases/tag/",
    crate_version!()
);
