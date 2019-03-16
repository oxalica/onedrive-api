//! Resource Objects defined in the OneDrive API.
//!
//! # Field descriper
//!
//! Some structures have field descriper mods with singleton types representing
//! all controlable fields of it, which may be used
//! in [`onedrive_api::query_option`][query_option] to select or expand it using
//! `with_option` version API of [`DriveClient`][drive_client].
//!
//! ## Example
//! Here is an example to use [`resource::DriveItemField`][drive_item_field].
//! ```
//! use onedrive_api::{DriveClient, ItemLocation, query_option::ObjectOption};
//! use onedrive_api::resource::DriveItemField;
//!
//! // let client: DriveClient;
//! # fn run(client: &DriveClient) -> onedrive_api::Result<()> {
//! let item = client
//!     .get_item_with_option(
//!         ItemLocation::root(),
//!         None,
//!         // Only response `id` and `e_tag` to reduce data transmission.
//!         ObjectOption::new()
//!             .select(&[&DriveItemField::id, &DriveItemField::e_tag]),
//!     )?;
//!
//! Ok(())
//! # }
//! ```
//!
//!
//! # See also
//! [Microsoft Docs](https://docs.microsoft.com/en-us/onedrive/developer/rest-api/resources/?view=odsp-graph-online)
//!
//! [query_option]: ../query_option/index.html
//! [drive_client]: ../struct.DriveClient.html
//! [drive_item_field]: ./DriveItemField/index.html
use serde::{Deserialize, Serialize};

/// A semantic alias for URL string in resource objects.
pub type Url = String;

macro_rules! define_string_wrapper {
    ($($(#[$meta:meta])* $vis:vis $name:ident;)*) => { $(
        $(#[$meta])*
        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        $vis struct $name(String);

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
    /// The tag from `DriveItem::c_tag` (TODO) is for the content of the item,
    /// while the one from [`DriveItem::e_tag`][e_tag] is for the entire item (metadata + content).
    ///
    /// [e_tag]: ./struct.DriveItem.html#structfield.e_tag
    pub Tag;
}

#[doc(hidden)]
pub trait ResourceFieldOf<T> {
    fn api_field_name(&self) -> String;
}

// Separate `type` to enable making `ResoucrFieldOf` into trait object.
#[doc(hidden)]
pub trait ResourceFieldTypeOf<T>: ResourceFieldOf<T> {
    type Type;
}

macro_rules! define_resource_object {
    ($(
        $(#[$meta:meta])*
        $vis:vis struct $struct_name:ident $(#$field_mod_name:ident)? {
            $(
                $(#[$field_meta:meta])*
                $([unselectable $($unselectable_mark:ident)?])?
                pub $field_name:ident $(@$field_rename:literal)?: Option<$field_ty:ty>,
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
                    $(#[serde(rename = $field_rename)])?
                    pub $field_name: Option<$field_ty>,
                )*
                #[serde(default)]
                _private: (),
            }

            define_resource_object! { __impl_struct($struct_name $($field_mod_name)?) [
                $({
                    [$(unselectable $($unselectable_mark)?)?]
                    [$($field_meta)*]
                    $field_name
                    ($($field_rename)?)
                    ($field_ty)
                })*
            ] }
        )*
    };
    (__impl_struct($struct_name:ident) $tt:tt) => {}; // No field mod.
    (__impl_struct($struct_name:ident $field_mod_name:ident) [$({
        [$($unselectable:ident)?]
        [$($meta:meta)*]
        $field:ident
        ($($rename:literal)?)
        ($ty:ty)
    })*]) => {
        /// Fields descriptors.
        ///
        /// More details in [mod documentation][mod].
        ///
        /// [mod]: ../index.html
        #[allow(non_snake_case)]
        pub mod $field_mod_name {
            $(
                define_resource_object! { __impl_if_empty($($unselectable)?) {
                    /// Field descriptor.
                    ///
                    /// More details in [mod documentation][mod].
                    ///
                    /// [mod]: ../index.html
                    #[allow(non_camel_case_types)]
                    pub struct $field;

                    impl ::std::fmt::Debug for $field {
                        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                            f.debug_struct(
                                concat!(stringify!($field_mod_name), "::", stringify!($field)),
                            ).finish()
                        }
                    }
                } }
            )*
        }

        $(
            define_resource_object! { __impl_if_empty($($unselectable)?) {
                impl ResourceFieldOf<$struct_name> for $field_mod_name::$field {
                    #[inline]
                    fn api_field_name(&self) -> String {
                        snake_to_camel_case(stringify!($field))
                        $(; $rename)? // Replace
                    }
                }

                impl ResourceFieldTypeOf<$struct_name> for $field_mod_name::$field {
                    type Type = $ty;
                }
            } }
        )*
    };
    (__impl_if_empty() { $($tt:tt)* }) => { $($tt)* };
    (__impl_if_empty($sth:tt) $tt:tt) => {};
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
        // TODO: Incomplete
        pub id: Option<DriveId>,
        // created_by: IdentitySet,
        // created_date_time: Timestamp,
        pub description: Option<String>,
        // drive_type: DriveType,
        pub items: Option<Vec<DriveItem>>,
        // last_modified_by: IdeneitySet,
        // last_modified_date_time: Timestamp,
        pub name: Option<String>,
        // owner: IdentitySet,
        // quota: Quota,
        // root: DriveItem,
        // sharepoint_ids: SharepointIds,
        pub special: Option<DriveItem>,
        // system: SystemFacet,
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
        // TODO: Incomplete
        // Type specified fields

        // audio: Audio,
        // content: Stream,
        // file: File,
        // folder: Folder,
        // image: Image,
        // location: Option<GeoCoordinations>,
        // malware: Option<Malware>,
        // package: Package,
        // photo: Photo,
        // root: Root,
        // special_folder: SpecialFolder,
        // video: Video,

        // Drive item
        // c_tag: Option<Tag>,
        pub deleted: Option<Deleted>,
        pub description: Option<String>,
        // pub file_system_info: FileSystemInfo,
        // publication: Option<PublicationFacet>,
        // remote_item: Option<RemoteItem>,
        // search_result: Option<SearchResult>,
        // shared: Shared,
        // sharepoint_ids: SharepointIds,
        pub size: Option<i64>,
        // web_dav_url: Url,

        // Relationships

        // activities: Vec<ItemActivity>,
        pub children: Option<Vec<DriveItem>>,
        // permissions: Vec<Permission>,
        // thumbnails: Vec<ThumbnailSet>,
        // versions: Vec<DriveItemVersion>,

        // Base item
        pub id: Option<ItemId>,
        // created_by: IdentitySet,
        // created_date_time: Timestamp,
        pub e_tag: Option<Tag>,
        // last_modified_by: IdentitySet,
        // last_modified_date_time: Timestamp,
        pub name: Option<String>,
        pub parent_reference: Option<ItemReference>,
        pub web_url: Option<Url>,

        // Instance annotations
        [unselectable]
        pub download_url @"@microsoft.graph.downloadUrl": Option<Url>,
    }

    /// Deleted facet
    ///
    /// The `Deleted` resource indicates that the item has been deleted.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/deleted?view=graph-rest-1.0)
    #[derive(Debug, Serialize)]
    pub struct Deleted {
        pub state: Option<String>,
    }

    /// ItemReference resource type
    ///
    /// The `ItemReference` resource provides information necessary to address a `DriveItem` via the API.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/itemreference?view=graph-rest-1.0)
    #[derive(Debug, Serialize)]
    pub struct ItemReference {
        pub drive_id: Option<DriveId>,
        // drive_type: DriveType,
        pub id: Option<ItemId>,
        // list_id: String,
        pub name: Option<String>,
        pub path: Option<String>,
        // shared_id: String,
        // sharepoint_ids: SharepointIds,
        // site_id: String,
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
}
