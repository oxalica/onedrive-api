//! # onedrive-api
//!
//! The `onedrive-api` crate provides a middle-level HTTP [`Client`][client] to the
//! [OneDrive][onedrive] API through [Microsoft Graph][graph], and also [`AuthClient`][auth_client]
//! with utilities for authorization to it.
//!
//! The [`onedrive_api::DriveClient`][client] and [`onedrive_api::AuthClient`][auth_client]
//! are synchronous by using `reqwest::Client`. Async support is TODO.
//!
//! ## Example
//! ```
//! use onedrive_api::{DriveClient, FileName, DriveLocation, ItemLocation};
//! # fn run() -> onedrive_api::Result<()> {
//!
//! let client = DriveClient::new(
//!     "<...TOKEN...>".to_owned(), // Login token to Microsoft Graph.
//!     DriveLocation::me(),
//! );
//! let folder_item = client.create_folder(
//!     ItemLocation::root(),
//!     FileName::new("test_folder").unwrap(),
//! )?;
//! client.upload_small(
//!     folder_item.id.as_ref().unwrap(),
//!     b"Hello, world",
//! )?;
//!
//! Ok(())
//! # }
//! ```
//!
//! [client]: ./struct.DriveClient.html
//! [auth_client]: ./struct.AuthClient.html
//! [onedrive]: https://onedrive.live.com/about
//! [graph]: https://docs.microsoft.com/graph/overview
#![deny(warnings)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

mod authorization;
mod client;
mod error;
pub mod query_option;
pub mod resource;
mod util;

pub use self::authorization::{AuthClient, Permission, Token};
pub use self::client::{
    DriveClient, ExpectRange, ListChildrenFetcher, TrackChangeFetcher, UploadSession,
};
pub use self::error::{Error, Result};
pub use self::resource::{DriveId, ItemId, Tag};
pub use self::util::{DriveLocation, FileName, ItemLocation};
