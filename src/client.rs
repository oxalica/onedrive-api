use crate::error::{Error, Result};
use crate::option::{CollectionOption, DriveItemPutOption, ObjectOption};
use crate::resource::*;
use crate::util::*;
use crate::{ConflictBehavior, ExpectRange};
use serde::{Deserialize, Serialize};

macro_rules! api_url {
    (@$init:expr; $($seg:expr),* $(,)*) => {
        {
            let mut url = ::url::Url::parse($init).unwrap();
            {
                let mut buf = url.path_segments_mut().unwrap();
                $(ApiPathComponent::extend_into($seg, &mut buf);)*
            } // End borrowing of `url`
            url
        }
    };
    ($($t:tt)*) => {
        api_url!(@"https://graph.microsoft.com/v1.0"; $($t)*)
    };
}

macro_rules! api_path {
    ($($t:tt)*) => {
        api_url![@"path://"; $($t)*].path()
    };
}

/// The authorized client to access OneDrive resources in a specified Drive.
#[derive(Debug)]
pub struct DriveClient {
    client: ::reqwest::Client,
    token: String,
    drive: DriveLocation,
}

impl DriveClient {
    /// Create a DriveClient to perform operations in a Drive.
    pub fn new(token: String, drive: impl Into<DriveLocation>) -> Self {
        DriveClient {
            client: ::reqwest::Client::new(),
            token,
            drive: drive.into(),
        }
    }

    /// Get `Drive`.
    ///
    /// Retrieve the properties and relationships of a [`Drive`][drive] resource.
    ///
    /// # See also
    /// [`resource::Drive`][drive]
    ///
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/drive-get?view=graph-rest-1.0)
    ///
    /// [drive]: ./resource/struct.Drive.html
    pub fn get_drive_with_option(&self, option: ObjectOption<DriveField>) -> Result<Drive> {
        self.client
            .get(api_url![&self.drive])
            .apply(&option)
            .bearer_auth(&self.token)
            .send()?
            .parse()
    }

    /// Shortcut to `get_drive_with_option` with default parameters.
    ///
    /// # See also
    /// [`get_drive_with_option`][with_opt]
    ///
    /// [with_opt]: #method.get_drive_with_option
    pub fn get_drive(&self) -> Result<Drive> {
        self.get_drive_with_option(Default::default())
    }

    /// List children of a [`DriveItem`][drive_item].
    ///
    /// Return a collection of [`DriveItem`][drive_item]s in the children relationship
    /// of the given one.
    ///
    /// # Note
    /// Will return `Ok(None)` if `if_none_match` is set and matches the item.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-list-children?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    pub fn list_children_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        option: CollectionOption<DriveItemField>,
    ) -> Result<Option<ListChildrenFetcher>> {
        self.client
            .get(api_url![&self.drive, &item.into(), "children"])
            .apply(&option)
            .bearer_auth(&self.token)
            .send()?
            .parse_optional()
            .map(|opt_resp| opt_resp.map(|resp| ListChildrenFetcher::new(self, resp)))
    }

    /// Shortcut to `list_children_with_option` with default params and fetch all.
    ///
    /// # See also
    /// [`list_children_with_option`][with_opt]
    ///
    /// [with_opt]: #method.list_children_with_option
    pub fn list_children<'a>(&self, item: impl Into<ItemLocation<'a>>) -> Result<Vec<DriveItem>> {
        self.list_children_with_option(item.into(), Default::default())?
            .unwrap()
            .fetch_all()
    }

    /// Get a [`DriveItem`][drive_item] resource.
    ///
    /// Retrieve the metadata for a [`DriveItem`][drive_item] by file system path or ID.
    ///
    /// # Errors
    /// Will return `Ok(None)` if `if_none_match` is set and matches the item .
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-get?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    pub fn get_item_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        option: ObjectOption<DriveItemField>,
    ) -> Result<Option<DriveItem>> {
        self.client
            .get(api_url![&self.drive, &item.into()])
            .apply(&option)
            .bearer_auth(&self.token)
            .send()?
            .parse_optional()
    }

    /// Shortcut to `get_item_with_option` with default parameters.
    ///
    /// # See also
    /// [`get_item_with_option`][with_opt]
    ///
    /// [with_opt]: #method.get_item_with_option
    pub fn get_item<'a>(&self, item: impl Into<ItemLocation<'a>>) -> Result<DriveItem> {
        self.get_item_with_option(item.into(), Default::default())
            .map(|v| v.unwrap())
    }

    /// Create a new folder in a drive
    ///
    /// Create a new folder [`DriveItem`][drive_item] with a specified parent item or path.
    ///
    /// # Errors
    /// Will return `Err` with HTTP CONFLICT if `conflict_behavior` is `Fail` and
    /// the target already exists.
    ///
    /// # Note
    /// `conflict_behavior` is supported.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-post-children?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    pub fn create_folder_with_option<'a>(
        &self,
        parent_item: impl Into<ItemLocation<'a>>,
        name: &FileName,
        option: DriveItemPutOption,
    ) -> Result<DriveItem> {
        #[derive(Serialize)]
        struct Folder {}

        #[derive(Serialize)]
        struct Request<'a> {
            name: &'a str,
            folder: Folder,
            // https://docs.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0#instance-attributes
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: ConflictBehavior,
        }

        self.client
            .post(api_url![&self.drive, &parent_item.into(), "children"])
            .bearer_auth(&self.token)
            .apply(&option)
            .json(&Request {
                name: name.as_str(),
                folder: Folder {},
                conflict_behavior: option
                    .get_conflict_behavior()
                    .unwrap_or(ConflictBehavior::Fail),
            })
            .send()?
            .parse()
    }

    /// Shortcut to `create_folder_with_option` with default parameters.
    ///
    /// # See also
    /// [`create_folder_with_option`][with_opt]
    ///
    /// [with_opt]: #method.create_folder_with_option
    pub fn create_folder<'a>(
        &self,
        parent_item: impl Into<ItemLocation<'a>>,
        name: &FileName,
    ) -> Result<DriveItem> {
        self.create_folder_with_option(parent_item.into(), name, Default::default())
    }

    const UPLOAD_SMALL_LIMIT: usize = 4_000_000; // 4 MB

    /// Upload or replace the contents of a [`DriveItem`][drive_item]
    ///
    /// The simple upload API allows you to provide the contents of a new file or
    /// update the contents of an existing file in a single API call. This method
    /// only supports files up to 4MB in size.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-put-content?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    pub fn upload_small<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        data: &[u8],
    ) -> Result<DriveItem> {
        assert!(
            data.len() <= Self::UPLOAD_SMALL_LIMIT,
            "Data too large for upload_small ({} B > {} B)",
            data.len(),
            Self::UPLOAD_SMALL_LIMIT,
        );

        self.client
            .put(api_url![&self.drive, &item.into(), "content"])
            .bearer_auth(&self.token)
            .body(data.to_owned())
            .send()?
            .parse()
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
    /// Will return `Err` with HTTP PRECONDITION_FAILED if `if_match` is set
    /// but does not match the item.
    ///
    /// # Note
    /// `conflict_behavior` is supported.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#create-an-upload-session)
    pub fn new_upload_session_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        option: DriveItemPutOption,
    ) -> Result<UploadSession> {
        #[derive(Serialize)]
        struct Item {
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: ConflictBehavior,
        }

        #[derive(Serialize)]
        struct Request {
            item: Item,
        }

        self.client
            .post(api_url![&self.drive, &item.into(), "createUploadSession"])
            .apply(&option)
            .bearer_auth(&self.token)
            .json(&Request {
                item: Item {
                    conflict_behavior: option
                        .get_conflict_behavior()
                        .unwrap_or(ConflictBehavior::Fail),
                },
            })
            .send()?
            .parse()
    }

    /// Shortcut to `new_upload_session_with_option` with `ConflictBehavior::Fail`.
    ///
    /// # See also
    /// [`new_upload_session_with_option`][with_opt]
    ///
    /// [with_opt]: #method.create_folder_with_option
    pub fn new_upload_session<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
    ) -> Result<UploadSession> {
        self.new_upload_session_with_option(item.into(), Default::default())
    }

    /// Resuming an in-progress upload
    ///
    /// Query the status of the upload to find out which byte ranges
    /// have been received previously.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#resuming-an-in-progress-upload)
    pub fn get_upload_session(&self, upload_url: &str) -> Result<UploadSession> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct UploadSessionResponse {
            // There is no url.
            next_expected_ranges: Vec<ExpectRange>,
            expiration_date_time: TimestampString,
        }

        self.client
            .get(upload_url)
            .send()?
            .parse::<UploadSessionResponse>()
            .map(|resp| UploadSession {
                upload_url: upload_url.to_owned(),
                next_expected_ranges: resp.next_expected_ranges,
                expiration_date_time: resp.expiration_date_time,
            })
    }

    /// Cancel the upload session
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
    pub fn delete_upload_session(&self, sess: &UploadSession) -> Result<()> {
        self.client
            .delete(&sess.upload_url)
            .send()?
            .parse_no_content()
    }

    const UPLOAD_SESSION_PART_LIMIT: usize = 60 << 20; // 60 MiB

    /// Upload bytes to the upload session
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
    /// # Returns
    /// - If error occurs, will return `Err`.
    /// - If the part is uploaded successfully, but the file is not complete yet,
    ///   will return `Ok(None)`.
    /// - If this is the last part and it is uploaded successfully,
    ///   will return `Ok(Some(newly_created_drive_item))`.
    ///
    /// # Errors
    /// When the file is completely uploaded, if an item with the same name is created
    /// during uploading, the last `upload_to_session` call will return `Err` with
    /// HTTP CONFLICT.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-createuploadsession?view=graph-rest-1.0#upload-bytes-to-the-upload-session)
    pub fn upload_to_session(
        &self,
        session: &UploadSession,
        data: &[u8],
        remote_range: ::std::ops::Range<usize>,
        total_size: usize,
    ) -> Result<Option<DriveItem>> {
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

        self.client
            .put(&session.upload_url)
            // No auth token
            .header(
                ::reqwest::header::CONTENT_RANGE,
                format!(
                    "bytes {}-{}/{}",
                    remote_range.start,
                    remote_range.end - 1,
                    total_size
                ),
            )
            .body(data.to_owned())
            .send()?
            .parse_optional()
    }

    /// Copy a DriveItem.
    ///
    /// Asynchronously creates a copy of an driveItem (including any children),
    /// under a new parent item or with a new name.
    ///
    /// # Note
    /// The conflict behavior is not mentioned in Microsoft Docs.
    ///
    /// But it seems to be `rename` if the destination folder is just the current
    /// parent folder, and `fail` if not.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-copy?view=graph-rest-1.0)
    pub fn copy<'a, 'b>(
        &self,
        source_item: impl Into<ItemLocation<'a>>,
        dest_folder: impl Into<ItemLocation<'b>>,
        dest_name: &FileName,
    ) -> Result<CopyProgressMonitor> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            parent_reference: ItemReference<'a>,
            name: &'a str,
        }

        let url = self
            .client
            .post(api_url![&self.drive, &source_item.into(), "copy"])
            .bearer_auth(&self.token)
            .json(&Request {
                parent_reference: ItemReference {
                    path: api_path![&self.drive, &dest_folder.into()],
                },
                name: dest_name.as_str(),
            })
            .send()?
            .check_status()?
            .headers()
            .get(::reqwest::header::LOCATION)
            .ok_or_else(|| {
                Error::unexpected_response("Header `Location` not exists in response of `copy`")
            })?
            .to_str()
            .map_err(|_| Error::unexpected_response("Invalid string header `Location`"))?
            .to_owned();

        Ok(CopyProgressMonitor::from_url(&self.client, url))
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
    /// `conflict_behavior` is supported.
    ///
    /// # Errors
    /// Will return `Err` with HTTP PRECONDITION_FAILED if `if_match` is set
    /// but doesn't match the item.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-move?view=graph-rest-1.0)
    pub fn move_with_option<'a, 'b>(
        &self,
        source_item: impl Into<ItemLocation<'a>>,
        dest_folder: impl Into<ItemLocation<'b>>,
        dest_name: Option<&FileName>,
        option: DriveItemPutOption,
    ) -> Result<DriveItem> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            parent_reference: ItemReference<'a>,
            name: Option<&'a str>,
            #[serde(rename = "@microsoft.graph.conflictBehavior")]
            conflict_behavior: ConflictBehavior,
        }

        self.client
            .patch(api_url![&self.drive, &source_item.into()])
            .bearer_auth(&self.token)
            .apply(&option)
            .json(&Request {
                parent_reference: ItemReference {
                    path: api_path![&self.drive, &dest_folder.into()],
                },
                name: dest_name.map(FileName::as_str),
                conflict_behavior: option
                    .get_conflict_behavior()
                    .unwrap_or(ConflictBehavior::Fail),
            })
            .send()?
            .parse()
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
    ) -> Result<DriveItem> {
        self.move_with_option(
            source_item.into(),
            dest_folder.into(),
            dest_name,
            Default::default(),
        )
    }

    /// Delete a [`DriveItem`][drive_item].
    ///
    /// Delete a [`DriveItem`][drive_item] by using its ID or path. Note that deleting items using
    /// this method will move the items to the recycle bin instead of permanently
    /// deleting the item.
    ///
    /// # Errors
    /// Will return `Err` with HTTP PRECONDITION_FAILED if `if_match` is set but
    /// does not match the item.
    ///
    /// # Note
    /// `conflict_behavior` is **NOT*** supported.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delete?view=graph-rest-1.0)
    ///
    /// [drive_item]: ./resource/struct.DriveItem.html
    pub fn delete_with_option<'a>(
        &self,
        item: impl Into<ItemLocation<'a>>,
        option: DriveItemPutOption,
    ) -> Result<()> {
        assert!(
            option.get_conflict_behavior().is_none(),
            "`conflict_behavior` is not supported by `delete[_with_option]`",
        );

        self.client
            .delete(api_url![&self.drive, &item.into()])
            .bearer_auth(&self.token)
            .apply(&option)
            .send()?
            .parse_no_content()
    }

    /// Shortcut to `delete_with_option`.
    ///
    /// # See also
    /// [`delete_with_option`][with_opt]
    ///
    /// [with_opt]: #method.delete_with_option
    pub fn delete<'a>(&self, item: impl Into<ItemLocation<'a>>) -> Result<()> {
        self.delete_with_option(item.into(), Default::default())
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
    /// # Return
    /// The fetcher for fetching all changes from initial state (empty) to the snapshot of
    /// current states.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0)
    pub fn track_changes_from_initial_with_option<'a>(
        &self,
        folder: impl Into<ItemLocation<'a>>,
        option: CollectionOption<DriveItemField>,
    ) -> Result<TrackChangeFetcher> {
        self.client
            .get(&api_url![&self.drive, &folder.into(), "delta"].into_string())
            .apply(&option)
            .bearer_auth(&self.token)
            .send()?
            .parse()
            .map(|resp| TrackChangeFetcher::new(self, resp))
    }

    /// Shortcut to `track_changes_from_initial_with_option` with default parameters.
    ///
    /// # See also
    /// [`track_changes_from_initial_with_option`][with_opt]
    ///
    /// [with_opt]: #method.track_changes_from_initial_with_option
    pub fn track_changes_from_initial<'a>(
        &self,
        folder: impl Into<ItemLocation<'a>>,
    ) -> Result<(Vec<DriveItem>, String)> {
        self.track_changes_from_initial_with_option(folder.into(), Default::default())?
            .fetch_all()
    }

    /// Track changes for a folder from snapshot (delta url) to snapshot of current states.
    ///
    /// # See also
    /// [`DriveClient::track_changes_from_initial_with_option`][track_initial]
    ///
    /// [track_initial]: #method.track_changes_from_initial_with_option
    pub fn track_changes_from_delta_url(&self, delta_url: &str) -> Result<TrackChangeFetcher> {
        self.client
            .get(delta_url)
            .bearer_auth(&self.token)
            .send()?
            .parse()
            .map(|resp| TrackChangeFetcher::new(self, resp))
    }

    /// Get a delta url representing the snapshot of current states.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0)
    pub fn get_latest_delta_url<'a>(&self, folder: impl Into<ItemLocation<'a>>) -> Result<String> {
        self.client
            .get(&api_url![&self.drive, &folder.into(), "delta"].into_string())
            .query(&[("token", "latest")])
            .bearer_auth(&self.token)
            .send()?
            .parse()
            .and_then(|resp: DriveItemCollectionResponse| {
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
/// [`DriveClient::copy`][copy]
///
/// [Microsoft docs](https://docs.microsoft.com/en-us/graph/long-running-actions-overview)
///
/// [copy]: ./struct.DriveClient.html#method.copy
#[derive(Debug)]
pub struct CopyProgressMonitor {
    client: ::reqwest::Client,
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
/// [Microsoft Docs Beta](https://docs.microsoft.com/en-us/graph/api/resources/asyncjobstatus?view=graph-rest-beta#json-representation)
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

impl CopyProgressMonitor {
    /// Make a progress monitor using existing `url`.
    ///
    /// The `url` must be get from [`CopyProgressMonitor::get_url`][get_url]
    ///
    /// [get_url]: #method.get_url
    pub fn from_url(client: &::reqwest::Client, url: String) -> Self {
        Self {
            client: client.clone(),
            url,
        }
    }

    /// Get the url of this monitor.
    pub fn get_url(&self) -> &str {
        &self.url
    }

    /// Fetch the `copy` progress.
    ///
    /// # See also
    /// [`CopyProgress`][copy_progress]
    ///
    /// [copy_progress]: ../struct.CopyProgress.html
    pub fn fetch_progress(&self) -> Result<CopyProgress> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            operation: String,
            percentage_complete: f64,
            status: CopyStatus,
        }

        let resp: Response = self.client.get(&self.url).send()?.parse()?;

        if resp.operation != "ItemCopy" {
            return Err(Error::unexpected_response("Url is not for copy progress"));
        }

        Ok(CopyProgress {
            percentage_complete: resp.percentage_complete,
            status: resp.status,
            _private: (),
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
struct DriveItemFetcher {
    client: reqwest::Client,
    token: String,
    response: DriveItemCollectionResponse,
}

impl DriveItemFetcher {
    fn new(client: &DriveClient, response: DriveItemCollectionResponse) -> Self {
        Self {
            client: client.client.clone(),
            token: client.token.clone(),
            response,
        }
    }

    fn get_next_url(&self) -> Option<&str> {
        match (&self.response.value, &self.response.next_url) {
            (None, Some(url)) => Some(url),
            _ => None,
        }
    }

    fn fetch_next(&mut self) -> Option<Result<Vec<DriveItem>>> {
        if let Some(v) = self.response.value.take() {
            return Some(Ok(v));
        }

        // Not `take` here. Will remain unchanged if failed to fetch.
        let url = self.response.next_url.as_ref()?;
        match (|| {
            self.client
                .get(url)
                .bearer_auth(&self.token)
                .send()?
                .parse::<DriveItemCollectionResponse>()
        })() {
            Err(err) => Some(Err(err)),
            Ok(DriveItemCollectionResponse {
                next_url: Some(_),
                value: None,
                ..
            }) => Some(Err(Error::unexpected_response(
                "Missing field `value` when not finished",
            ))),
            Ok(resp) => {
                self.response = resp;
                Some(Ok(self.response.value.take()?))
            }
        }
    }

    fn fetch_all(&mut self) -> Result<Vec<DriveItem>> {
        let mut buf = vec![];
        while let Some(ret) = self.fetch_next() {
            buf.append(&mut ret?);
        }
        Ok(buf)
    }
}

/// The page fetcher for children listing operation with `Iterator` interface.
///
/// # See also
/// [`DriveClient::list_childre_with_option`][list_children_with_opt]
///
/// [list_children_with_opt]: ./struct.DriveClient.html#method.list_children_with_option
#[derive(Debug)]
pub struct ListChildrenFetcher {
    fetcher: DriveItemFetcher,
}

impl ListChildrenFetcher {
    fn new(client: &DriveClient, response: DriveItemCollectionResponse) -> Self {
        Self {
            fetcher: DriveItemFetcher::new(client, response),
        }
    }

    /// Resume a fetching process from url from
    /// [`ListChildrenFetcher::get_next_url`][get_next_url].
    ///
    /// [get_next_url]: #method.get_next_url
    pub fn resume_from(client: &DriveClient, next_url: String) -> Self {
        Self::new(
            client,
            DriveItemCollectionResponse {
                value: None,
                next_url: Some(next_url),
                delta_url: None,
            },
        )
    }

    /// Try to get the url to the next page.
    ///
    /// Used for resuming the fetching progress.
    ///
    /// # Error
    /// Will success only if there are more pages and the first page is already readed.
    ///
    /// # Note
    /// The first page data from [`DriveClient::list_children_with_option`][list_children_with_opt]
    /// will be cached and have no idempotent url to resume/re-fetch.
    ///
    /// [list_children_with_opt]: ./struct.DriveClient.html#method.list_children_with_option
    pub fn get_next_url(&self) -> Option<&str> {
        self.fetcher.get_next_url()
    }

    /// Fetch all rest pages and return all items concated.
    ///
    /// # Errors
    /// Will return `Err` if any error occurs during fetching.
    ///
    /// Note that you will lose all progress unless all requests are success,
    /// so it is preferred to use `Iterator::next` to make it more error-tolerant.
    pub fn fetch_all(mut self) -> Result<Vec<DriveItem>> {
        self.fetcher.fetch_all()
    }
}

impl Iterator for ListChildrenFetcher {
    type Item = Result<Vec<DriveItem>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.fetcher.fetch_next()
    }
}

/// The page fetcher for tracking operations with `Iterator` interface.
///
/// # See also
/// [`DriveClient::track_changes_from_initial`][track_initial]
///
/// [`DriveClient::track_changes_from_delta_url`][track_delta]
///
/// [track_initial]: ./struct.DriveClient.html#method.track_changes_from_initial_with_option
/// [track_delta]: ./struct.DriveClient.html#method.track_changes_from_delta_url
#[derive(Debug)]
pub struct TrackChangeFetcher {
    fetcher: DriveItemFetcher,
}

impl TrackChangeFetcher {
    fn new(client: &DriveClient, response: DriveItemCollectionResponse) -> Self {
        Self {
            fetcher: DriveItemFetcher::new(client, response),
        }
    }

    /// Resume a fetching process from url.
    ///
    /// The url should be from [`TrackChangeFetcher::get_next_url`][get_next_url].
    ///
    /// [get_next_url]: #method.get_next_url
    pub fn resume_from(client: &DriveClient, next_url: String) -> Self {
        Self {
            fetcher: DriveItemFetcher {
                client: client.client.clone(),
                token: client.token.clone(),
                response: DriveItemCollectionResponse {
                    value: None,
                    delta_url: None,
                    next_url: Some(next_url),
                },
            },
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
    /// [`DriveClient::track_changes_from_initial_with_option`][track_initial]
    /// will be cached and have no idempotent url to resume/re-fetch.
    ///
    /// [track_initial]: ./struct.DriveClient.html#method.track_changes_from_initial
    pub fn get_next_url(&self) -> Option<&str> {
        self.fetcher.get_next_url()
    }

    /// Try to the delta url representing a snapshot of current track change operation.
    ///
    /// Used for tracking changes from this snapshot (rather than initial) later,
    /// using [`DriveClient::track_changes_from_delta_url`][track_delta].
    ///
    /// # Error
    /// Will success only if there are no more pages.
    ///
    /// # See also
    /// [`DriveClient::track_changes_from_delta_url`][track_delta]
    ///
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0#example-last-page-in-a-set)
    ///
    /// [track_delta]: ./struct.DriveClient.html#method.track_changes_from_delta_url
    pub fn get_delta_url(&self) -> Option<&str> {
        match &self.fetcher.response {
            DriveItemCollectionResponse {
                value: None,
                delta_url: Some(url),
                ..
            } => Some(url),
            _ => None,
        }
    }

    /// Fetch all rest pages and return all items concated with a delta url.
    ///
    /// # Errors
    /// Will return `Err` if any error occurs during fetching.
    ///
    /// Note that you will lose all progress unless all requests are success,
    /// so it is preferred to use `Iterator::next` to make it more error-tolerant.
    pub fn fetch_all(mut self) -> Result<(Vec<DriveItem>, String)> {
        let v = self.fetcher.fetch_all()?;
        // Must not be None if `fetch_all` succeed.
        Ok((v, self.fetcher.response.delta_url.unwrap()))
    }
}

impl Iterator for TrackChangeFetcher {
    type Item = Result<Vec<DriveItem>>;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.fetcher.fetch_next(), &self.fetcher.response.delta_url) {
            (None, None) => Some(Err(Error::unexpected_response(
                "Missing field `@odata.deltaLink` for the last page",
            ))),
            (ret, _) => ret,
        }
    }
}

#[derive(Serialize)]
struct ItemReference<'a> {
    path: &'a str,
}

/// An upload session for resumable file uploading process.
///
/// # See also
/// [`DriveClient::new_upload_session`][get_session]
///
/// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0)
///
/// [get_session]: ./struct.DriveClient.html#method.new_upload_session
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
    /// It is preferred to use [`DriveClient::get_upload_session`][get_session] to get
    /// the upload session and then [`DriveClient::upload_to_session`][upload_to_session] to
    /// perform upload.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0#properties)
    ///
    /// [get_session]: ./struct.DriveClient.html#method.get_upload_session
    /// [upload_to_session]: ./struct.DriveClient.html#method.upload_to_session
    pub fn get_url(&self) -> &str {
        &self.upload_url
    }

    /// Get a collection of byte ranges that the server is missing for the file.
    ///
    /// Used for determine what to upload when resuming a session.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0#properties)
    pub fn get_next_expected_ranges(&self) -> &[ExpectRange] {
        &self.next_expected_ranges
    }

    /// Get the date and time in UTC that the upload session will expire.
    ///
    /// The complete file must be uploaded before this expiration time is reached.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/uploadsession?view=graph-rest-1.0#properties)
    pub fn get_expiration_date_time(&self) -> &TimestampString {
        &self.expiration_date_time
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_api_url() {
        assert_eq!(
            api_url!["a", &DriveLocation::me(), "b"].path(),
            "/v1.0/a/drive/b",
        );

        let mock_drive_id = DriveId::new("1234".to_owned());
        assert_eq!(
            api_path![&DriveLocation::from_id(mock_drive_id)],
            "/drives/1234",
        );

        assert_eq!(
            api_path![&ItemLocation::from_path("/dir/file name").unwrap()],
            "/root:%2Fdir%2Ffile%20name:",
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
