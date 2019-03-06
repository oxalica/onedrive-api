#![deny(warnings)]
#![deny(missing_debug_implementations)]

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
