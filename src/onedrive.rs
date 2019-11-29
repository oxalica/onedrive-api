use crate::{
    api::{self, Api, ApiExt as _, SimpleApi},
    error::{Error, Result},
    option::{CollectionOption, DriveItemPutOption, ObjectOption},
    resource::*,
    util::{
        ApiPathComponent, DriveLocation, FileName, ItemLocation, RequestBuilder as Builder,
        ResponseExt as _,
    },
    {ConflictBehavior, ExpectRange},
};
use http::{header, Method};
use serde::{Deserialize, Serialize};
use url::Url;

macro_rules! api_url {
    (@$init:expr; $($seg:expr),* $(,)? $(; $bind:ident => $extra:block)?) => {{
        let mut url = Url::parse($init).unwrap();
        {
            let mut buf = url.path_segments_mut().unwrap();
            $(ApiPathComponent::extend_into($seg, &mut buf);)*
        } // End borrowing of `url`
        $({
            let $bind = &mut url;
            $extra;
        })?
        url
    }};
    ($($t:tt)*) => {
        api_url!(@"https://graph.microsoft.com/v1.0"; $($t)*)
    };
}

/// FIXME: More efficient impl.
macro_rules! api_path {
    ($item:expr) => {{
        let mut url = Url::parse("path:///drive").unwrap();
        let item: &ItemLocation = $item;
        ApiPathComponent::extend_into(item, &mut url.path_segments_mut().unwrap());
        url
    }
    .path()};
}

/// The authorized client to access OneDrive resources in a specified Drive.
#[derive(Debug)]
pub struct OneDrive {
    token: String,
    drive: DriveLocation,
}

impl OneDrive {
    /// Create a new OneDrive instance with token to perform operations in a Drive.
    pub fn new(token: String, drive: impl Into<DriveLocation>) -> Self {
        OneDrive {
            token,
            drive: drive.into(),
        }
    }

    /// Get the token used to create the OneDrive instance.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get current `Drive`.
    ///
    /// Retrieve the properties and relationships of a [`resource::Drive`][drive] resource.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0)
    ///
    /// [drive]: ./resource/struct.Drive.html
    pub fn get_drive_with_option(
        &self,
        option: ObjectOption<DriveField>,
    ) -> impl Api<Response = Drive> {
        SimpleApi::new(
            Builder::new(Method::GET, api_url![&self.drive])
                .apply(option)
                .bearer_auth(&self.token)
                .empty_body(),
        )
        .and_then(|resp| resp.parse())
    }

    /// Shortcut to `get_drive_with_option` with default parameters.
    ///
    /// # See also
    /// [`get_drive_with_option`][with_opt]
    ///
    /// [with_opt]: #method.get_drive_with_option
    pub fn get_drive(&self) -> impl Api<Response = Drive> {
        self.get_drive_with_option(Default::default())
    }

    /// List children of a `DriveItem`.
    ///
    /// Retrieve a collection of [`resource::DriveItem`][drive_item]s in the children relationship
    /// of the given one.
    ///
    /// # Response
    /// If successful, respond a [fetcher][fetcher] for fetching changes from initial state (empty) to the snapshot of
    /// current states. See [`ListChildrenFetcher`][fetcher] for more details.
    ///
    /// If [`if_none_match`][if_none_match] is set and it matches the item tag, return an `None`.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-list-children?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    /// [if_none_match]: ./option/struct.CollectionOption.html#method.if_none_match
    /// [fetcher]: ./struct.ListChildrenFetcher.html
    pub fn list_children_with_option<'t, 'a>(
        &'t self,
        item: impl Into<ItemLocation<'a>>,
        option: CollectionOption<DriveItemField>,
    ) -> impl Api<Response = Option<ListChildrenFetcher<'t>>> {
        SimpleApi::new(
            Builder::new(Method::GET, api_url![&self.drive, &item.into(), "children"])
                .apply(option)
                .bearer_auth(&self.token)
                .empty_body(),
        )
        .and_then(move |resp| {
            let opt_resp: Option<DriveItemCollectionResponse> = resp.parse_optional()?;
            Ok(opt_resp.map(move |resp| ListChildrenFetcher::new(&self.token, resp)))
        })
    }

    /// Shortcut to `list_children_with_option` with default params,
    /// and fetch and collect all children.
    ///
    /// # See also
    /// [`list_children_with_option`][with_opt]
    ///
    /// [with_opt]: #method.list_children_with_option
    // FIXME: https://github.com/rust-lang/rust/issues/42940
    pub fn list_children<'a, 's>(
        &'s self,
        item: impl Into<ItemLocation<'a>> + 's,
    ) -> impl Api<Response = Vec<DriveItem>> + 's {
        struct ListChildren<A> {
            api: A,
        }

        impl<A> api::sealed::Sealed for ListChildren<A> {}

        impl<'b, A: Api<Response = Option<ListChildrenFetcher<'b>>>> Api for ListChildren<A> {
            type Response = Vec<DriveItem>;

            fn execute(self, client: &impl api::Client) -> Result<Self::Response> {
                let mut fetcher = self.api.execute(client)?.unwrap();
                let mut buf = vec![];
                while let Some(chunk) = fetcher.fetch_next_page().execute(client)? {
                    buf.extend(chunk);
                }
                Ok(buf)
            }
        }

        ListChildren {
            api: self.list_children_with_option(item, Default::default()),
        }
    }

    /// Get a `DriveItem` resource.
    ///
    /// Retrieve the metadata for a [`resource::DriveItem`][drive_item] by file system path or ID.
    ///
    /// # Errors
    /// Will return `Ok(None)` if [`if_none_match`][if_none_match] is set and it matches the item tag.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-get?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    /// [if_none_match]: ./option/struct.CollectionOption.html#method.if_none_match
    pub fn get_item_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        option: ObjectOption<DriveItemField>,
    ) -> impl Api<Response = Option<DriveItem>> {
        SimpleApi::new(
            Builder::new(Method::GET, api_url![&self.drive, &item.into()])
                .apply(option)
                .bearer_auth(&self.token)
                .empty_body(),
        )
        .and_then(|resp| resp.parse_optional())
    }

    /// Shortcut to `get_item_with_option` with default parameters.
    ///
    /// # See also
    /// [`get_item_with_option`][with_opt]
    ///
    /// [with_opt]: #method.get_item_with_option
    pub fn get_item<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
    ) -> impl Api<Response = DriveItem> {
        self.get_item_with_option(item, Default::default())
            .and_then(|v| Ok(v.expect("Empty response while If-None-Match is not specified")))
    }

    /// Create a new folder under an DriveItem
    ///
    /// Create a new folder [`DriveItem`][drive_item] with a specified parent item or path.
    ///
    /// # Errors
    /// Will result in `Err` with HTTP 409 CONFLICT if [`conflict_behavior`][conflict_behavior]
    /// is set to [`Fail`][conflict_fail] and the target already exists.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-post-children?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    /// [conflict_behavior]: ./option/struct.DriveItemPutOption.html#method.conflict_behavior
    /// [conflict_fail]: ./enum.ConflictBehavior.html#variant.Fail
    pub fn create_folder_with_option<'a>(
        &self,
        parent_item: impl Into<ItemLocation<'a>>,
        name: &FileName,
        option: DriveItemPutOption,
    ) -> impl Api<Response = DriveItem> {
        #[derive(Serialize)]
        struct Folder {}

        #[derive(Serialize)]
        struct Req<'a> {
            name: &'a str,
            folder: Folder,
            // https://docs.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0#instance-attributes
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: ConflictBehavior,
        }

        let conflict_behavior = option
            .get_conflict_behavior()
            .unwrap_or(ConflictBehavior::Fail);
        SimpleApi::new(
            Builder::new(
                Method::POST,
                api_url![&self.drive, &parent_item.into(), "children"],
            )
            .bearer_auth(&self.token)
            .apply(option)
            .json_body(&Req {
                name: name.as_str(),
                folder: Folder {},
                conflict_behavior,
            }),
        )
        .and_then(|resp| resp.parse())
    }

    /// Shortcut to `create_folder_with_option` with default options.
    ///
    /// # See also
    /// [`create_folder_with_option`][with_opt]
    ///
    /// [with_opt]: #method.create_folder_with_option
    pub fn create_folder<'a>(
        &self,
        parent_item: impl Into<ItemLocation<'a>>,
        name: &FileName,
    ) -> impl Api<Response = DriveItem> {
        self.create_folder_with_option(parent_item, name, Default::default())
    }

    /// Update DriveItem properties
    ///
    /// Update the metadata for a [`DriveItem`][drive_item].
    ///
    /// If you want to rename or move an [`DriveItem`][drive_item] to another place,
    /// you should use [`move_`][move_] (or [`move_with_option`][move_with_opt]) instead of this, which is a wrapper
    /// to this API endpoint to make things easier.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-update?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    /// [move_]: #method.move_
    /// [move_with_opt]: #method.move_with_option
    pub fn update_item_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        patch: &DriveItem,
        option: ObjectOption<DriveItemField>,
    ) -> impl Api<Response = DriveItem> {
        SimpleApi::new(
            Builder::new(Method::PATCH, api_url![&self.drive, &item.into()])
                .bearer_auth(&self.token)
                .apply(option)
                .json_body(patch),
        )
        .and_then(|resp| resp.parse())
    }

    /// Shortcut to `update_item_with_option` with default options.
    ///
    /// # See also
    /// [`update_item_with_option`][with_opt]
    ///
    /// [with_opt]: #method.update_item_with_option
    pub fn update_item<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        patch: &DriveItem,
    ) -> impl Api<Response = DriveItem> {
        self.update_item_with_option(item, patch, Default::default())
    }

    const UPLOAD_SMALL_LIMIT: usize = 4_000_000; // 4 MB

    /// Upload or replace the contents of a `DriveItem` file.
    ///
    /// The simple upload API allows you to provide the contents of a new file or
    /// update the contents of an existing file in a single API call. This method
    /// only supports files up to 4MB in size.
    ///
    /// # Panic
    /// Panic if `data` is larger than 4 MB (4,000,000 bytes).
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-put-content?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    pub fn upload_small<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        data: &[u8],
    ) -> impl Api<Response = DriveItem> {
        assert!(
            data.len() <= Self::UPLOAD_SMALL_LIMIT,
            "Data too large for upload_small ({} B > {} B)",
            data.len(),
            Self::UPLOAD_SMALL_LIMIT,
        );

        SimpleApi::new(
            Builder::new(Method::PUT, api_url![&self.drive, &item.into(), "content"])
                .bearer_auth(&self.token)
                .bytes_body(data.to_owned())
                .map_err(Into::into),
        )
        .and_then(|resp| resp.parse())
    }

    /// Create an upload session.
    ///
    /// Create an upload session to allow your app to upload files up to
    /// the maximum file size. An upload session allows your app to
    /// upload ranges of the file in sequential API requests, which allows
    /// the transfer to be resumed if a connection is dropped
    /// while the upload is in progress.
    ///
    /// # Errors
    /// Will return `Err` with HTTP 412 PRECONDITION_FAILED if [`if_match`][if_match] is set
    /// but does not match the item.
    ///
    /// # Note
    /// [`conflict_behavior`][conflict_behavior] is supported.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#create-an-upload-session)
    ///
    /// [if_match]: ./option/struct.CollectionOption.html#method.if_match
    /// [conflict_behavior]: ./option/struct.DriveItemPutOption.html#method.conflict_behavior
    pub fn new_upload_session_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        option: DriveItemPutOption,
    ) -> impl Api<Response = UploadSession> {
        #[derive(Serialize)]
        struct Item {
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: ConflictBehavior,
        }

        #[derive(Serialize)]
        struct Req {
            item: Item,
        }

        let conflict_behavior = option
            .get_conflict_behavior()
            .unwrap_or(ConflictBehavior::Fail);
        SimpleApi::new(
            Builder::new(
                Method::POST,
                api_url![&self.drive, &item.into(), "createUploadSession"],
            )
            .apply(option)
            .bearer_auth(&self.token)
            .json_body(&Req {
                item: Item { conflict_behavior },
            }),
        )
        .and_then(|resp| resp.parse())
    }

    /// Shortcut to `new_upload_session_with_option` with `ConflictBehavior::Fail`.
    ///
    /// # See also
    /// [`new_upload_session_with_option`][with_opt]
    ///
    /// [with_opt]: #method.new_upload_session_with_option
    pub fn new_upload_session<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
    ) -> impl Api<Response = UploadSession> {
        self.new_upload_session_with_option(item, Default::default())
    }

    /// Resuming an in-progress upload
    ///
    /// Query the status of the upload to find out which byte ranges
    /// have been received previously.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#resuming-an-in-progress-upload)
    pub fn get_upload_session(&self, upload_url: &str) -> impl Api<Response = UploadSession> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            // There is no url.
            next_expected_ranges: Vec<ExpectRange>,
            expiration_date_time: TimestampString,
        }

        let upload_url = upload_url.to_owned();
        SimpleApi::new(Builder::new(Method::GET, &upload_url).empty_body()).and_then(move |resp| {
            let resp: Resp = resp.parse()?;
            Ok(UploadSession {
                upload_url,
                next_expected_ranges: resp.next_expected_ranges,
                expiration_date_time: resp.expiration_date_time,
            })
        })
    }

    /// Cancel an upload session
    ///
    /// This cleans up the temporary file holding the data previously uploaded.
    /// This should be used in scenarios where the upload is aborted, for example,
    /// if the user cancels the transfer.
    ///
    /// Temporary files and their accompanying upload session are automatically
    /// cleaned up after the expirationDateTime has passed. Temporary files may
    /// not be deleted immedately after the expiration time has elapsed.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#cancel-the-upload-session)
    pub fn delete_upload_session(&self, sess: &UploadSession) -> impl Api<Response = ()> {
        SimpleApi::new(Builder::new(Method::DELETE, &sess.upload_url).empty_body())
            .and_then(|resp| resp.parse_no_content())
    }

    const UPLOAD_SESSION_PART_LIMIT: usize = 60 << 20; // 60 MiB

    /// Upload bytes to an upload session
    ///
    /// You can upload the entire file, or split the file into multiple byte ranges,
    /// as long as the maximum bytes in any given request is less than 60 MiB.
    /// The fragments of the file must be uploaded sequentially in order. Uploading
    /// fragments out of order will result in an error.
    ///
    /// Note: If your app splits a file into multiple byte ranges, the size of each
    /// byte range MUST be a multiple of 320 KiB (327,680 bytes). Using a fragment
    /// size that does not divide evenly by 320 KiB will result in errors committing
    /// some files.
    ///
    /// # Response
    /// - If the part is uploaded successfully, but the file is not complete yet,
    ///   will respond `None`.
    /// - If this is the last part and it is uploaded successfully,
    ///   will return `Some(<newly_created_drive_item>)`.
    ///
    /// # Error
    /// When the file is completely uploaded, if an item with the same name is created
    /// during uploading, the last `upload_to_session` call will return `Err` with
    /// HTTP 409 CONFLICT.
    ///
    /// # Panic
    /// Panic if `remote_range` is invalid, not match the length of `data`, or
    /// `data` is larger than 60 MiB (62,914,560 bytes).
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#upload-bytes-to-the-upload-session)
    pub fn upload_to_session(
        &self,
        session: &UploadSession,
        data: &[u8],
        remote_range: ::std::ops::Range<usize>,
        total_size: usize,
    ) -> impl Api<Response = Option<DriveItem>> {
        // FIXME: https://github.com/rust-lang/rust-clippy/issues/3807
        #[allow(clippy::len_zero)]
        {
            assert!(
                remote_range.len() > 0 && remote_range.end <= total_size,
                "Invalid range",
            );
        }
        assert_eq!(
            data.len(),
            remote_range.end - remote_range.start,
            "Length mismatch"
        );
        assert!(
            data.len() <= Self::UPLOAD_SESSION_PART_LIMIT,
            "Data too large for one part ({} B > {} B)",
            data.len(),
            Self::UPLOAD_SESSION_PART_LIMIT,
        );

        SimpleApi::new(
            Builder::new(Method::PUT, &session.upload_url)
                // No auth token
                .header(
                    header::CONTENT_RANGE,
                    format!(
                        "bytes {}-{}/{}",
                        remote_range.start,
                        remote_range.end - 1,
                        total_size
                    ),
                )
                .bytes_body(data.to_owned()),
        )
        .and_then(|resp| resp.parse_optional())
    }

    /// Copy a DriveItem.
    ///
    /// Asynchronously creates a copy of an driveItem (including any children),
    /// under a new parent item or with a new name.
    ///
    /// # Note
    /// The conflict behavior is not mentioned in Microsoft Docs, and cannot be specified.
    ///
    /// But it seems to behave as [`Rename`][conflict_rename] if the destination folder is just the current
    /// parent folder, and [`Fail`][conflict_fail] overwise.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-copy?view=graph-rest-1.0)
    ///
    /// [conflict_rename]: ./enum.ConflictBehavior.html#variant.Rename
    /// [conflict_fail]: ./enum.ConflictBehavior.html#variant.Fail
    pub fn copy<'a, 'b>(
        &self,
        source_item: impl Into<ItemLocation<'a>>,
        dest_folder: impl Into<ItemLocation<'b>>,
        dest_name: &FileName,
    ) -> impl Api<Response = CopyProgressMonitor> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Req<'a> {
            parent_reference: ItemReference<'a>,
            name: &'a str,
        }

        SimpleApi::new(
            Builder::new(
                Method::POST,
                api_url![&self.drive, &source_item.into(), "copy"],
            )
            .bearer_auth(&self.token)
            .json_body(&Req {
                parent_reference: ItemReference {
                    path: api_path!(&dest_folder.into()),
                },
                name: dest_name.as_str(),
            }),
        )
        .and_then(move |resp| {
            let url = resp
                .check_status()?
                .headers()
                .get(header::LOCATION)
                .ok_or_else(|| {
                    Error::unexpected_response("Header `Location` not exists in response of `copy`")
                })?
                .to_str()
                .map_err(|_| Error::unexpected_response("Invalid string header `Location`"))?
                .to_owned();
            Ok(CopyProgressMonitor::from_url(&self.token, url))
        })
    }

    /// Move a DriveItem to a new folder.
    ///
    /// This is a special case of the Update method. Your app can combine
    /// moving an item to a new container and updating other properties of
    /// the item into a single request.
    ///
    /// Note: Items cannot be moved between Drives using this request.
    ///
    /// # Note
    /// [`conflict_behavior`][conflict_behavior] is supported.
    ///
    /// # Errors
    /// Will return `Err` with HTTP 412 PRECONDITION_FAILED if [`if_match`][if_match] is set
    /// but it does not match the item.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-move?view=graph-rest-1.0)
    ///
    /// [conflict_behavior]: ./option/struct.DriveItemPutOption.html#method.conflict_behavior
    /// [if_match]: ./option/struct.CollectionOption.html#method.if_match
    pub fn move_with_option<'a, 'b>(
        &self,
        source_item: impl Into<ItemLocation<'a>>,
        dest_folder: impl Into<ItemLocation<'b>>,
        dest_name: Option<&FileName>,
        option: DriveItemPutOption,
    ) -> impl Api<Response = DriveItem> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Req<'a> {
            parent_reference: ItemReference<'a>,
            name: Option<&'a str>,
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: ConflictBehavior,
        }

        let conflict_behavior = option
            .get_conflict_behavior()
            .unwrap_or(ConflictBehavior::Fail);
        SimpleApi::new(
            Builder::new(Method::PATCH, api_url![&self.drive, &source_item.into()])
                .bearer_auth(&self.token)
                .apply(option)
                .json_body(&Req {
                    parent_reference: ItemReference {
                        path: api_path!(&dest_folder.into()),
                    },
                    name: dest_name.map(FileName::as_str),
                    conflict_behavior,
                }),
        )
        .and_then(|resp| resp.parse())
    }

    /// Shortcut to `move_with_option` with `ConflictBehavior::Fail`.
    ///
    /// # See also
    /// [`move_with_option`][with_opt]
    ///
    /// [with_opt]: #method.move_with_option
    pub fn move_<'a, 'b>(
        &self,
        source_item: impl Into<ItemLocation<'a>>,
        dest_folder: impl Into<ItemLocation<'b>>,
        dest_name: Option<&FileName>,
    ) -> impl Api<Response = DriveItem> {
        self.move_with_option(source_item, dest_folder, dest_name, Default::default())
    }

    /// Delete a `DriveItem`.
    ///
    /// Delete a [`DriveItem`][drive_item] by using its ID or path. Note that deleting items using
    /// this method will move the items to the recycle bin instead of permanently
    /// deleting the item.
    ///
    /// # Error
    /// Will result in error with HTTP 412 PRECONDITION_FAILED if [`if_match`][if_match] is set but
    /// does not match the item.
    ///
    /// # Panic
    /// [`conflict_behavior`][conflict_behavior] is **NOT** supported. Set it will cause a panic.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delete?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    /// [if_match]: ./option/struct.CollectionOption.html#method.if_match
    /// [conflict_behavior]: ./option/struct.DriveItemPutOption.html#method.conflict_behavior
    pub fn delete_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        option: DriveItemPutOption,
    ) -> impl Api<Response = ()> {
        assert!(
            option.get_conflict_behavior().is_none(),
            "`conflict_behavior` is not supported by `delete[_with_option]`",
        );

        SimpleApi::new(
            Builder::new(Method::DELETE, api_url![&self.drive, &item.into()])
                .bearer_auth(&self.token)
                .apply(option)
                .empty_body(),
        )
        .and_then(|resp| resp.parse_no_content())
    }

    /// Shortcut to `delete_with_option`.
    ///
    /// # See also
    /// [`delete_with_option`][with_opt]
    ///
    /// [with_opt]: #method.delete_with_option
    pub fn delete<'a>(&self, item: impl Into<ItemLocation<'a>>) -> impl Api<Response = ()> {
        self.delete_with_option(item, Default::default())
    }

    /// Track changes for a folder from initial state (empty state) to snapshot of current states.
    ///
    /// This method allows your app to track changes to a drive and its children over time.
    /// Deleted items are returned with the deleted facet. Items with this property set
    /// should be removed from your local state.
    ///
    /// Note: you should only delete a folder locally if it is empty after
    /// syncing all the changes.
    ///
    /// # Response
    /// Respond a [fetcher][fetcher] for fetching changes from initial state (empty) to the snapshot of
    /// current states. See [`TrackChangeFetcher`][fetcher] for more details.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0)
    ///
    /// [fetcher]: ./struct.TrackChangeFetcher.html
    pub fn track_changes_from_initial_with_option<'a>(
        &self,
        folder: impl Into<ItemLocation<'a>>,
        option: CollectionOption<DriveItemField>,
    ) -> impl Api<Response = TrackChangeFetcher<'_>> {
        SimpleApi::new(
            Builder::new(Method::GET, api_url![&self.drive, &folder.into(), "delta"])
                .apply(option)
                .bearer_auth(&self.token)
                .empty_body(),
        )
        .and_then(move |resp| {
            let resp: DriveItemCollectionResponse = resp.parse()?;
            Ok(TrackChangeFetcher::new(&self.token, resp))
        })
    }

    /// Shortcut to `track_changes_from_initial_with_option` with default parameters.
    ///
    /// # See also
    /// [`track_changes_from_initial_with_option`][with_opt]
    ///
    /// [`TrackChangeFetcher`][fetcher]
    ///
    /// [with_opt]: #method.track_changes_from_initial_with_option
    /// [fetcher]: ./struct.TrackChangeFetcher.html
    pub fn track_changes_from_initial<'a>(
        &self,
        folder: impl Into<ItemLocation<'a>>,
    ) -> impl Api<Response = TrackChangeFetcher<'_>> {
        self.track_changes_from_initial_with_option(folder, Default::default())
    }

    /// Track changes for a folder from snapshot (delta url) to snapshot of current states.
    ///
    /// # See also
    /// [`OneDrive::track_changes_from_initial_with_option`][track_initial]
    ///
    /// [`TrackChangeFetcher`][fetcher]
    ///
    /// [track_initial]: #method.track_changes_from_initial_with_option
    /// [fetcher]: ./struct.TrackChangeFetcher.html
    pub fn track_changes_from_delta_url<'t>(
        &'t self,
        delta_url: &str,
    ) -> impl Api<Response = TrackChangeFetcher<'_>> {
        SimpleApi::new(
            Builder::new(Method::GET, delta_url)
                .bearer_auth(&self.token)
                .empty_body(),
        )
        .and_then(move |resp| {
            let resp: DriveItemCollectionResponse = resp.parse()?;
            Ok(TrackChangeFetcher::new(&self.token, resp))
        })
    }

    /// Get a delta url representing the snapshot of current states.
    ///
    /// The delta url can be used in [`track_changes_from_delta_url`][track_from_delta] later
    /// to get diffs between two snapshots of states.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0#retrieving-the-current-deltalink)
    ///
    /// [track_from_delta]: #method.track_changes_from_delta_url
    pub fn get_latest_delta_url<'a>(
        &self,
        folder: impl Into<ItemLocation<'a>>,
    ) -> impl Api<Response = String> {
        SimpleApi::new(
            Builder::new(
                Method::GET,
                api_url![
                    &self.drive, &folder.into(), "delta";
                    url => {
                        url.query_pairs_mut().append_pair("token", "latest");
                    }
                ],
            )
            .bearer_auth(&self.token)
            .empty_body(),
        )
        .and_then(|resp| {
            let resp: DriveItemCollectionResponse = resp.parse()?;
            resp.delta_url.ok_or_else(|| {
                Error::unexpected_response(
                    "Missing field `@odata.deltaLink` for getting latest delta",
                )
            })
        })
    }
}

/// The monitor for checking the progress of a asynchronous `copy` operation.
///
/// # See also
/// [`OneDrive::copy`][copy]
///
/// [Microsoft docs](https://docs.microsoft.com/en-us/graph/long-running-actions-overview)
///
/// [copy]: ./struct.OneDrive.html#method.copy
#[derive(Debug)]
pub struct CopyProgressMonitor<'t> {
    token: &'t str,
    url: String,
}

/// The progress of a asynchronous `copy` operation.
///
/// # See also
/// [Microsoft Docs Beta](https://docs.microsoft.com/en-us/graph/api/resources/asyncjobstatus?view=graph-rest-beta)
#[allow(missing_docs)]
#[derive(Debug)]
pub struct CopyProgress {
    pub percentage_complete: f64,
    pub status: CopyStatus,
    _private: (),
}

/// The status of a `copy` operation.
///
/// # See also
/// [`CopyProgress`][copy_progress]
///
/// [Microsoft Docs Beta](https://docs.microsoft.com/en-us/graph/api/resources/asyncjobstatus?view=graph-rest-beta#json-representation)
///
/// [copy_progress]: ./struct.CopyProgress.html
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CopyStatus {
    NotStarted,
    InProgress,
    Completed,
    Updating,
    Failed,
    DeletePending,
    DeleteFailed,
    Waiting,
}

impl<'t> CopyProgressMonitor<'t> {
    /// Make a progress monitor using existing `url`.
    ///
    /// The `url` must be get from [`CopyProgressMonitor::url`][url]
    ///
    /// [url]: #method.url
    pub fn from_url(token: &'t str, url: String) -> Self {
        Self { token, url }
    }

    /// Get the url of this monitor.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Fetch the `copy` progress.
    ///
    /// # See also
    /// [`CopyProgress`][copy_progress]
    ///
    /// [copy_progress]: ../struct.CopyProgress.html
    pub fn fetch_progress(&self) -> impl Api<Response = CopyProgress> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            percentage_complete: f64,
            status: CopyStatus,
        }

        SimpleApi::new(Builder::new(Method::GET, &self.url).empty_body())
            .and_then(|resp| resp.parse())
            .and_then(|resp: Resp| {
                Ok(CopyProgress {
                    percentage_complete: resp.percentage_complete,
                    status: resp.status,
                    _private: (),
                })
            })
    }
}

#[derive(Debug, Deserialize)]
struct DriveItemCollectionResponse {
    value: Option<Vec<DriveItem>>,
    #[serde(rename = "@odata.nextLink")]
    next_url: Option<String>,
    #[serde(rename = "@odata.deltaLink")]
    delta_url: Option<String>,
}

#[derive(Debug)]
struct DriveItemFetcher<'t> {
    token: &'t str,
    last_response: DriveItemCollectionResponse,
}

impl<'t> DriveItemFetcher<'t> {
    fn new(token: &'t str, first_response: DriveItemCollectionResponse) -> Self {
        Self {
            token,
            last_response: first_response,
        }
    }

    fn resume_from(token: &'t str, next_url: String) -> Self {
        Self::new(
            token,
            DriveItemCollectionResponse {
                value: None,
                next_url: Some(next_url),
                delta_url: None,
            },
        )
    }

    fn next_url(&self) -> Option<&str> {
        // Return `None` for the first page, or it will
        // lost items of the first page when resumed.
        match &self.last_response {
            DriveItemCollectionResponse {
                value: None,
                next_url: Some(next_url),
                ..
            } => Some(next_url),
            _ => None,
        }
    }

    fn delta_url(&self) -> Option<&str> {
        self.last_response.delta_url.as_ref().map(|s| &**s)
    }

    fn fetch_next_page(&mut self) -> impl Api<Response = Option<Vec<DriveItem>>> + '_ {
        struct FetchNextPage<'s> {
            token: &'s str,
            last_response: &'s mut DriveItemCollectionResponse,
        }

        impl api::sealed::Sealed for FetchNextPage<'_> {}

        impl Api for FetchNextPage<'_> {
            type Response = Option<Vec<DriveItem>>;

            fn execute(self, client: &impl api::Client) -> Result<Self::Response> {
                if let Some(items) = self.last_response.value.take() {
                    return Ok(Some(items));
                }
                let req = match self.last_response.next_url.as_ref() {
                    None => return Ok(None),
                    Some(url) => Builder::new(Method::GET, url)
                        .bearer_auth(self.token)
                        .empty_body()?,
                };
                let resp = client.execute_api(req)?;
                *self.last_response = resp.parse()?;
                Ok(Some(self.last_response.value.take().unwrap_or_default()))
            }
        }

        FetchNextPage {
            token: &self.token,
            last_response: &mut self.last_response,
        }
    }

    fn fetch_all(self) -> impl Api<Response = (Vec<DriveItem>, Option<String>)> + 't {
        struct FetchAll<'t> {
            fetcher: DriveItemFetcher<'t>,
        }

        impl api::sealed::Sealed for FetchAll<'_> {}

        impl Api for FetchAll<'_> {
            type Response = (Vec<DriveItem>, Option<String>);

            fn execute(mut self, client: &impl api::Client) -> Result<Self::Response> {
                let mut buf = vec![];
                while let Some(items) = self.fetcher.fetch_next_page().execute(client)? {
                    buf.extend(items);
                }
                Ok((buf, self.fetcher.delta_url().map(|s| s.to_owned())))
            }
        }

        FetchAll { fetcher: self }
    }
}

/// The page fetcher for listing children
///
/// # See also
/// [`OneDrive::list_children_with_option`][list_children_with_opt]
///
/// [list_children_with_opt]: ./struct.OneDrive.html#method.list_children_with_option
#[derive(Debug)]
pub struct ListChildrenFetcher<'t> {
    fetcher: DriveItemFetcher<'t>,
}

impl<'t> ListChildrenFetcher<'t> {
    fn new(token: &'t str, first_response: DriveItemCollectionResponse) -> Self {
        Self {
            fetcher: DriveItemFetcher::new(token, first_response),
        }
    }

    /// Resume a fetching process from url from
    /// [`ListChildrenFetcher::next_url`][next_url].
    ///
    /// [next_url]: #method.next_url
    pub fn resume_from(token: &'t str, next_url: String) -> Self {
        Self {
            fetcher: DriveItemFetcher::resume_from(token, next_url),
        }
    }

    /// Try to get the url to the next page.
    ///
    /// Used for resuming the fetching progress.
    ///
    /// # Error
    /// Will success only if there are more pages and the first page is already readed.
    ///
    /// # Note
    /// The first page data from [`OneDrive::list_children_with_option`][list_children_with_opt]
    /// will be cached and have no idempotent url to resume/re-fetch.
    ///
    /// [list_children_with_opt]: ./struct.OneDrive.html#method.list_children_with_option
    pub fn next_url(&self) -> Option<&str> {
        self.fetcher.next_url()
    }

    /// Fetch the next page, or `None` if reaches the end.
    pub fn fetch_next_page(&mut self) -> impl Api<Response = Option<Vec<DriveItem>>> + '_ {
        self.fetcher.fetch_next_page()
    }

    /// Fetch all rest pages and collect all items.
    ///
    /// # Errors
    ///
    /// Any error occurs when fetching will lead to an failure, and
    /// all progress will be lost.
    pub fn fetch_all(self) -> impl Api<Response = Vec<DriveItem>> + 't {
        self.fetcher.fetch_all().and_then(|(items, _)| Ok(items))
    }
}

/// The page fetcher for tracking operations with `Iterator` interface.
///
/// # See also
/// [`OneDrive::track_changes_from_initial`][track_initial]
///
/// [`OneDrive::track_changes_from_delta_url`][track_delta]
///
/// [track_initial]: ./struct.OneDrive.html#method.track_changes_from_initial_with_option
/// [track_delta]: ./struct.OneDrive.html#method.track_changes_from_delta_url
#[derive(Debug)]
pub struct TrackChangeFetcher<'t> {
    fetcher: DriveItemFetcher<'t>,
}

impl<'t> TrackChangeFetcher<'t> {
    fn new(token: &'t str, first_response: DriveItemCollectionResponse) -> Self {
        Self {
            fetcher: DriveItemFetcher::new(token, first_response),
        }
    }

    /// Resume a fetching process from url.
    ///
    /// The url should be from [`TrackChangeFetcher::next_url`][next_url].
    ///
    /// [next_url]: #method.next_url
    pub fn resume_from(token: &'t str, next_url: String) -> Self {
        Self {
            fetcher: DriveItemFetcher::resume_from(token, next_url),
        }
    }

    /// Try to get the url to the next page.
    ///
    /// Used for resuming the fetching progress.
    ///
    /// # Error
    /// Will success only if there are more pages and the first page is already readed.
    ///
    /// # Note
    /// The first page data from
    /// [`OneDrive::track_changes_from_initial_with_option`][track_initial]
    /// will be cached and have no idempotent url to resume/re-fetch.
    ///
    /// [track_initial]: ./struct.OneDrive.html#method.track_changes_from_initial
    pub fn next_url(&self) -> Option<&str> {
        self.fetcher.next_url()
    }

    /// Try to the delta url representing a snapshot of current track change operation.
    ///
    /// Used for tracking changes from this snapshot (rather than initial) later,
    /// using [`OneDrive::track_changes_from_delta_url`][track_delta].
    ///
    /// # Error
    /// Will success only if there are no more pages.
    ///
    /// # See also
    /// [`OneDrive::track_changes_from_delta_url`][track_delta]
    ///
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0#example-last-page-in-a-set)
    ///
    /// [track_delta]: ./struct.OneDrive.html#method.track_changes_from_delta_url
    pub fn delta_url(&self) -> Option<&str> {
        self.fetcher.delta_url()
    }

    /// Fetch the next page, or `None` if reaches the end.
    pub fn fetch_next_page(&mut self) -> impl Api<Response = Option<Vec<DriveItem>>> + '_ {
        self.fetcher.fetch_next_page()
    }

    /// Fetch all rest pages, collect all items, and also return `delta_url`.
    ///
    /// # Errors
    ///
    /// Any error occurs when fetching will lead to an failure, and
    /// all progress will be lost.
    pub fn fetch_all(self) -> impl Api<Response = (Vec<DriveItem>, String)> + 't {
        self.fetcher.fetch_all().and_then(|(items, opt_delta_url)| {
            let delta_url = opt_delta_url.ok_or_else(|| {
                Error::unexpected_response("Missing `@odata.deltaLink` for the last page")
            })?;
            Ok((items, delta_url))
        })
    }
}

#[derive(Serialize)]
struct ItemReference<'a> {
    path: &'a str,
}

/// An upload session for resumable file uploading process.
///
/// # See also
/// [`OneDrive::new_upload_session`][get_session]
///
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0)
///
/// [get_session]: ./struct.OneDrive.html#method.new_upload_session
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadSession {
    upload_url: String,
    next_expected_ranges: Vec<ExpectRange>,
    expiration_date_time: TimestampString,
}

impl UploadSession {
    /// The URL endpoint accepting PUT requests.
    ///
    /// Directly PUT to this URL is **NOT** encouraged.
    ///
    /// It is preferred to use [`OneDrive::get_upload_session`][get_session] to get
    /// the upload session and then [`OneDrive::upload_to_session`][upload_to_session] to
    /// perform upload.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0#properties)
    ///
    /// [get_session]: ./struct.OneDrive.html#method.get_upload_session
    /// [upload_to_session]: ./struct.OneDrive.html#method.upload_to_session
    pub fn upload_url(&self) -> &str {
        &self.upload_url
    }

    /// Get a collection of byte ranges that the server is missing for the file.
    ///
    /// Used for determine what to upload when resuming a session.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0#properties)
    pub fn next_expected_ranges(&self) -> &[ExpectRange] {
        &self.next_expected_ranges
    }

    /// Get the date and time in UTC that the upload session will expire.
    ///
    /// The complete file must be uploaded before this expiration time is reached.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0#properties)
    pub fn expiration_date_time(&self) -> &TimestampString {
        &self.expiration_date_time
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_api_url() {
        let mock_item_id = ItemId::new("1234".to_owned());
        assert_eq!(
            api_path!(&ItemLocation::from_id(&mock_item_id)),
            "/drive/items/1234",
        );

        assert_eq!(
            api_path!(&ItemLocation::from_path("/dir/file name").unwrap()),
            "/drive/root:%2Fdir%2Ffile%20name:",
        );
    }

    #[test]
    fn test_path_name_check() {
        let invalid_names = ["", ".*?", "a|b", "a<b>b", ":run", "/", "\\"];
        let valid_names = [
            "QAQ",
            "0",
            ".",
            "a-a：", // Unicode colon "\u{ff1a}"
            "魔理沙",
        ];

        let check_name = |s: &str| FileName::new(s).is_some();
        let check_path = |s: &str| ItemLocation::from_path(s).is_some();

        for s in &valid_names {
            assert!(check_name(s), "{}", s);
            let path = format!("/{}", s);
            assert!(check_path(&path), "{}", path);

            for s2 in &valid_names {
                let mut path = format!("/{}/{}", s, s2);
                assert!(check_path(&path), "{}", path);
                path.push('/'); // Trailing
                assert!(check_path(&path), "{}", path);
            }
        }

        for s in &invalid_names {
            assert!(!check_name(s), "{}", s);

            // `/` and `/xx/` is valid and is tested below.
            if s.is_empty() {
                continue;
            }

            let path = format!("/{}", s);
            assert!(!check_path(&path), "{}", path);

            for s2 in &valid_names {
                let path = format!("/{}/{}", s2, s);
                assert!(!check_path(&path), "{}", path);
            }
        }

        assert!(check_path("/"));
        assert!(check_path("/a"));
        assert!(check_path("/a/"));
        assert!(check_path("/a/b"));
        assert!(check_path("/a/b/"));

        assert!(!check_path(""));
        assert!(!check_path("/a/b//"));
        assert!(!check_path("a"));
        assert!(!check_path("a/"));
        assert!(!check_path("//"));
    }
}
