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
//! use onedrive_api::{OneDrive, ItemLocation, option::ObjectOption};
//! use onedrive_api::resource::*;
//!
//! # async fn run(drive: &OneDrive) -> onedrive_api::Result<()> {
//! // let drive: OneDrive;
//! let item: Option<DriveItem> = drive
//!     .get_item_with_option(
//!         ItemLocation::root(),
//!         ObjectOption::new()
//!             .if_none_match(&Tag("<abcdABCD1234>".to_owned()))
//!             // Only response `id` and `e_tag` to reduce data transmission.
//!             .select(&[DriveItemField::id, DriveItemField::e_tag]),
//!     )
//!     .await?;
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
use serde::{Deserialize, Serialize};

/// A semantic alias for URL string in resource objects.
pub type Url = String;

/// Boxed raw json value.
pub type JsonValue = Box<serde_json::Value>;

/// Timestamp string with ISO 8601 format.
pub type TimestampString = String;

macro_rules! define_string_wrapper {
    ($($(#[$meta:meta])* $vis:vis $name:ident;)*) => { $(
        $(#[$meta])*
        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        $vis struct $name(pub String);

        impl $name {
            /// View as str.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    )* };
}

define_string_wrapper! {
    /// Wrapper for a unique identifier to a `Drive`.
    ///
    /// # See also
    /// [Microsoft Docs: Drive resource type](https://docs.microsoft.com/en-us/graph/api/resources/drive?view=graph-rest-1.0)
    pub DriveId;

    /// Wrapper for a unique identifier for a `DriveItem`.
    ///
    /// # See also
    /// [Microsoft Docs: driveItem resource type](https://docs.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0)
    pub ItemId;

    /// Wrapper for a tag representing the state of an item.
    ///
    /// Used for avoid data transmission when a resource is not modified.
    ///
    /// The tag from [`DriveItem::c_tag`][c_tag] is for the content of the item,
    /// while the one from [`DriveItem::e_tag`][e_tag] is for the entire item (metadata + content).
    ///
    /// # See also
    /// [Microsoft Docs: driveItem resource type](https://docs.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0)
    ///
    /// [e_tag]: ./struct.DriveItem.html#structfield.e_tag
    /// [c_tag]: ./struct.DriveItem.html#structfield.c_tag
    pub Tag;
}

// Used for generalization over any resource field enums in `option`.
#[doc(hidden)]
pub trait ResourceField {
    fn __raw_name(&self) -> &'static str;
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
            #[derive(Debug, Default, Deserialize, Serialize)]
            #[serde(rename_all = "camelCase")]
            #[non_exhaustive]
            $vis struct $struct_name {
                $(
                    #[allow(missing_docs)]
                    #[serde(skip_serializing_if="Option::is_none")]
                    $(#[$field_meta])*
                    $($(#[serde(rename = $field_rename)])? pub $unsel_field_name)?
                    $(pub $sel_field_name)?
                        : Option<$field_ty>,
                )*
            }

            /// Fields descriptors.
            ///
            /// More details in [mod documentation][mod].
            ///
            /// [mod]: ./index.html
            #[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumVariantNames)]
            #[strum(serialize_all = "camelCase")]
            #[non_exhaustive]
            #[allow(missing_docs, non_camel_case_types)]
            $vis enum $field_enum_name {
                $(
                    $( // Only place selectable fields.
                        $sel_field_name,
                    )?
                )*
            }

            impl $field_enum_name {
                /// Get the raw camelCase name of the field.
                #[inline]
                pub fn raw_name(&self) -> &'static str {
                    <Self as strum::VariantNames>::VARIANTS[*self as usize]
                }
            }

            impl ResourceField for $field_enum_name {
                #[inline]
                fn __raw_name(&self) -> &'static str {
                    self.raw_name()
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

/// The error resource type, returned whenever an error occurs in the processing of a request.
///
/// Error responses follow the definition in the OData v4 specification for error responses.
///
/// **This struct is independent with [`OAuth2ErrorResponse`][oauth2_error_response] from OAuth2 API.**
///
/// It may be contained in [`onedrive_api::Error`][error] returned by storage API
/// (methods of [`OneDrive`][one_drive], [`ListChildrenFetcher`][list_children_fetcher], etc.).
///
/// # See also
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/errors#error-resource-type)
///
/// [oauth2_error_response]: ./struct.OAuth2ErrorResponse.html
/// [error]: ../struct.Error.html
/// [one_drive]: ../struct.OneDrive.html
/// [list_children_fetcher]: ../struct.ListChildrenFetcher.html
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct ErrorResponse {
    /// OData `code`. Non-exhaustive.
    ///
    /// Some possible values of `code` field can be found in:
    /// - [Error resource type: code property](https://docs.microsoft.com/en-us/graph/errors#code-property)
    /// - [Error codes for authorization endpoint errors](https://docs.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow#error-codes-for-authorization-endpoint-errors)
    /// - And maybe more.
    pub code: String,
    /// OData `message`. Usually to be human-readable.
    pub message: String,
    /// OData `innererror`. An optional object with additional or more specific error codes.
    #[serde(rename = "innererror")]
    pub inner_error: Option<serde_json::Map<String, serde_json::Value>>,
}

/// OAuth2 error response.
///
/// **This struct is independent with [`ErrorResponse`][error_response] from storage API.**
///
/// It can only be contained in [`onedrive_api::Error`][error] returned by operations
/// about OAuth2 (methods of [`Auth`][auth]).
///
/// # See also
/// - [Microsoft Docs: Request an authorization code](https://docs.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow#error-response)
/// - [Microsoft Docs: Request an access token](https://docs.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow#error-response-1)
/// - [Microsoft Docs: Refresh the access token](https://docs.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow#error-response-2)
///
/// [error_response]: ./struct.ErrorResponse.html
/// [error]: ../struct.Error.html
/// [auth]: ../struct.Auth.html
#[derive(Debug, Deserialize)]
#[allow(missing_docs)]
#[non_exhaustive]
pub struct OAuth2ErrorResponse {
    pub error: String,
    pub error_description: String,
    pub error_codes: Option<Vec<u32>>,
    pub timestamp: Option<String>,
    pub trace_id: Option<String>,
    pub correlation_id: Option<String>,
}

/// ```compile_fail
/// let _ = onedrive_api::resource::DriveItemField::download_url;
/// ```
fn _download_url_is_not_selectable() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_name() {
        assert_eq!(DriveField::id.raw_name(), "id");
        assert_eq!(DriveField::drive_type.raw_name(), "driveType");
        assert_eq!(DriveField::owner.raw_name(), "owner");
        assert_eq!(DriveField::web_url.raw_name(), "webUrl");

        assert_eq!(DriveItemField::id.raw_name(), "id");
        assert_eq!(
            DriveItemField::file_system_info.raw_name(),
            "fileSystemInfo"
        );
        assert_eq!(DriveItemField::size.raw_name(), "size");
        assert_eq!(DriveItemField::web_dav_url.raw_name(), "webDavUrl");
        assert_eq!(DriveItemField::web_url.raw_name(), "webUrl");
    }
}
