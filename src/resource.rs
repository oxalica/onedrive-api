//! Resource Objects defined in the OneDrive API.
//!
//! # Field descriptors
//!
//! Resource object `struct`s have field descriper `enum`s representing
//! all controlable fields of it, which may be used
//! in [`onedrive_api::option`][option] to [`select`][select] or [`expand`][expand] it using
//! `with_option` version API of [`OneDrive`][one_drive].
//!
//! ## Example
//! Here is an example to use [`resource::DriveItemField`][drive_item_field].
//! ```
//! use onedrive_api::{OneDrive, Api as _, ItemLocation, option::ObjectOption};
//! use onedrive_api::resource::*;
//!
//! # fn run(drive: &OneDrive, client: impl onedrive_api::Client) -> onedrive_api::Result<()> {
//! // let drive: OneDrive;
//! // let client: impl onedrive_api::Client;
//! let item: Option<DriveItem> = drive
//!     .get_item_with_option(
//!         ItemLocation::root(),
//!         ObjectOption::new()
//!             .if_none_match(&Tag::new("<abcdABCD1234>".to_owned()))
//!             // Only response `id` and `e_tag` to reduce data transmission.
//!             .select(&[DriveItemField::id, DriveItemField::e_tag]),
//!     )
//!     .execute(&client)?;
//! match item {
//!     None => println!("Tag matched"),
//!     Some(item) => {
//!         println!("id: {:?}, e_tag: {:?}", item.id.unwrap(), item.e_tag.unwrap());
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # See also
//! [Microsoft Docs](https://docs.microsoft.com/en-us/onedrive/developer/rest-api/resources/?view=odsp-graph-online)
//!
//! [option]: ../option/index.html
//! [select]: ../option/struct.ObjectOption.html#method.select
//! [expand]: ../option/struct.ObjectOption.html#method.expand
//! [one_drive]: ../struct.OneDrive.html
//! [drive_item_field]: ./enum.DriveItemField.html
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// A semantic alias for URL string in resource objects.
pub type Url = String;

/// Boxed raw json value.
pub type JsonValue = Box<::serde_json::Value>;

/// Timestamp string with ISO 8601 format.
pub type TimestampString = String;

macro_rules! define_string_wrapper {
    ($($(#[$meta:meta])* $vis:vis $name:ident;)*) => { $(
        $(#[$meta])*
        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        $vis struct $name(String);

        // TODO: Provide `to_string`
        impl $name {
            /// Wrap a string.
            ///
            /// Simply wrap without checking.
            pub fn new(id: String) -> Self {
                $name(id)
            }

            /// View as str.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    )* };
}

define_string_wrapper! {
    /// The unique identifier to a `Drive`.
    pub DriveId;

    /// The unique identifier for a `DriveItem`.
    pub ItemId;

    /// An tag representing the state of an item.
    ///
    /// Used for avoid data transmission when a resource is not modified.
    ///
    /// The tag from [`DriveItem::c_tag`][c_tag] is for the content of the item,
    /// while the one from [`DriveItem::e_tag`][e_tag] is for the entire item (metadata + content).
    ///
    /// [e_tag]: ./struct.DriveItem.html#structfield.e_tag
    /// [c_tag]: ./struct.DriveItem.html#structfield.c_tag
    pub Tag;
}

#[doc(hidden)]
pub trait ResourceField {
    fn api_field_name(&self) -> &'static str;
}

macro_rules! define_resource_object {
    ($(
        $(#[$meta:meta])*
        $vis:vis struct $struct_name:ident #$field_enum_name:ident {
            $(
                $(#[$field_meta:meta])*
                $([unselectable]
                    pub $unsel_field_name:ident
                    $(@$field_rename:literal)?
                )?
                $(pub $sel_field_name:ident)?
                    : Option<$field_ty:ty>,
            )*
        }
    )*) => {
        $(
            $(#[$meta])*
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            $vis struct $struct_name {
                $(
                    #[allow(missing_docs)]
                    $(#[$field_meta])*
                    $($(#[serde(rename = $field_rename)])? pub $unsel_field_name)?
                    $(pub $sel_field_name)?
                        : Option<$field_ty>,
                )*
                #[serde(default)]
                _private: (),
            }

            /// Fields descriptors.
            ///
            /// More details in [mod documentation][mod].
            ///
            /// [mod]: ./index.html
            // FIXME: Should be `#[non_exhaustive]`
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            $vis enum $field_enum_name {
                $(
                    $( // Only place selectable fields.
                        #[allow(missing_docs, non_camel_case_types)]
                        $sel_field_name,
                    )?
                )*
            }

            impl ResourceField for $field_enum_name {
                #[inline]
                fn api_field_name(&self) -> &'static str {
                    lazy_static! {
                        static ref FIELD_NAME_TABLE: Vec<String> = {
                            vec![
                                // Only place selectable fields.
                                $($(snake_to_camel_case(stringify!($sel_field_name)),)?)*
                            ]
                        };
                    }

                    &FIELD_NAME_TABLE[*self as usize]
                }
            }

        )*
    };
}

define_resource_object! {
    /// Drive resource type
    ///
    /// The drive resource is the top level object representing a user's OneDrive
    /// or a document library in SharePoint.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/drive?view=graph-rest-1.0)
    #[derive(Debug)]
    pub struct Drive #DriveField {
        pub id: Option<DriveId>,
        pub created_by: Option<JsonValue>,
        pub created_date_time: Option<TimestampString>,
        pub description: Option<String>,
        pub drive_type: Option<JsonValue>,
        pub items: Option<Vec<DriveItem>>,
        pub last_modified_by: Option<JsonValue>,
        pub last_modified_date_time: Option<TimestampString>,
        pub name: Option<String>,
        pub owner: Option<JsonValue>,
        pub quota: Option<JsonValue>,
        pub root: Option<DriveItem>,
        pub sharepoint_ids: Option<JsonValue>,
        pub special: Option<Vec<DriveItem>>,
        pub system: Option<JsonValue>,
        pub web_url: Option<Url>,
    }

    /// DriveItem resource type
    ///
    /// The `DriveItem` resource represents a file, folder, or other item stored in a drive.
    /// All file system objects in OneDrive and SharePoint are returned as `DriveItem` resources.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0)
    #[derive(Debug)]
    pub struct DriveItem #DriveItemField {

        // Drive item

        pub audio: Option<JsonValue>,
        pub content: Option<JsonValue>,
        pub c_tag: Option<Tag>,
        pub deleted: Option<JsonValue>,
        pub description: Option<String>,
        pub file: Option<JsonValue>,
        pub file_system_info: Option<JsonValue>,
        pub folder: Option<JsonValue>,
        pub image: Option<JsonValue>,
        pub location: Option<JsonValue>,
        pub package: Option<JsonValue>,
        pub photo: Option<JsonValue>,
        pub publication: Option<JsonValue>,
        pub remote_item: Option<JsonValue>,
        pub root: Option<JsonValue>,
        pub search_result: Option<JsonValue>,
        pub shared: Option<JsonValue>,
        pub sharepoint_ids: Option<JsonValue>,
        pub size: Option<i64>,
        pub special_folder: Option<JsonValue>,
        pub video: Option<JsonValue>,
        pub web_dav_url: Option<Url>,

        // Relationships

        pub children: Option<Vec<DriveItem>>,
        pub created_by_user: Option<JsonValue>,
        pub last_modified_by_user: Option<JsonValue>,
        pub permissions: Option<JsonValue>,
        pub thumbnails: Option<JsonValue>,
        pub versions: Option<JsonValue>,

        // Base item

        pub id: Option<ItemId>,
        pub created_by: Option<JsonValue>,
        pub created_date_time: Option<TimestampString>,
        pub e_tag: Option<Tag>,
        pub last_modified_by: Option<JsonValue>,
        pub last_modified_date_time: Option<TimestampString>,
        pub name: Option<String>,
        pub parent_reference: Option<JsonValue>,
        pub web_url: Option<Url>,

        // Instance annotations

        // `@microsoft.graph.conflictBehavior` is write-only.

        /// The pre-authorized url for downloading the content.
        ///
        /// It is **NOT** selectable through [`ObjectOption::select`][select] and
        /// only provided in the result of [`OneDrive::get_item`][get_item]
        /// (or [`OneDrive::get_item_with_option`][get_item_with_opt]).
        ///
        /// [select]: ../option/struct.ObjectOption.html#method.select
        /// [get_item]: ../struct.OneDrive.html#method.get_item
        /// [get_item_with_opt]: ../struct.OneDrive.html#method.get_item_with_option
        [unselectable]
        pub download_url @"@microsoft.graph.downloadUrl": Option<Url>,

        // `@microsoft.graph.sourceUrl` is write-only
    }
}

#[inline]
fn snake_to_camel_case(s: &str) -> String {
    let mut buf = String::new();
    let mut is_u = false;
    for c in s.chars() {
        if c == '_' {
            is_u = true;
        } else if is_u {
            is_u = false;
            buf.push(c.to_ascii_uppercase());
        } else {
            buf.push(c);
        }
    }
    buf
}

/// The error object with description and details of the error responsed from server.
///
/// It may be contained in [`onedrive_api::Error`][error] which may
/// be returned when processing requests.
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/errors#error-resource-type)
///
/// [error]: ../struct.Error.html
#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorObject {
    pub code: Option<String>,
    pub message: Option<String>,
    pub inner_error: Option<Box<ErrorObject>>,
    #[serde(flatten)]
    pub extra_data: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_to_camel_case() {
        let data = [
            ("abc", "abc"),
            ("hello_world", "helloWorld"),
            ("wh_tst_ef_ck", "whTstEfCk"),
        ];
        for (i, o) in &data {
            assert_eq!(snake_to_camel_case(i), *o);
        }
    }

    #[test]
    fn test_api_field_name() {
        assert_eq!(DriveField::id.api_field_name(), "id");
        assert_eq!(DriveField::drive_type.api_field_name(), "driveType");
        assert_eq!(DriveField::owner.api_field_name(), "owner");
        assert_eq!(DriveField::web_url.api_field_name(), "webUrl");

        assert_eq!(DriveItemField::id.api_field_name(), "id");
        assert_eq!(
            DriveItemField::file_system_info.api_field_name(),
            "fileSystemInfo"
        );
        assert_eq!(DriveItemField::size.api_field_name(), "size");
        assert_eq!(DriveItemField::web_dav_url.api_field_name(), "webDavUrl");
        assert_eq!(DriveItemField::web_url.api_field_name(), "webUrl");

        // This should fail to compile.
        // let _ = DriveItemField::download_url;
    }
}
