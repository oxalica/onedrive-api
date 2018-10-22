pub type Url = String;

pub type FileSize = u64;

/// The unique identifier for a `Drive`,
/// which can be get through `Client::get_drive`.
#[derive(Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct DriveId(String);

impl DriveId {
    pub fn new(id: String) -> Self {
        DriveId(id)
    }
}

impl AsRef<str> for DriveId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// The unique identifier for a drive,
/// which can be get through `Client::get_drive`.
#[derive(Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct ItemId(String);

impl ItemId {
    pub fn new(id: String) -> Self {
        ItemId(id)
    }
}

impl AsRef<str> for ItemId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// An eTag for the state of an item.
/// Used for avoid data transmission when a resource is not modified.
///
/// The tag from `DriveItem::c_tag` is for the content of the item,
/// while the one from `DriveItem::e_tag` is for the entire item (metadata + content).
#[derive(Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Tag(String);

impl Tag {
    pub fn new(tag: String) -> Self {
        Tag(tag)
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// The Drive resource.
///
/// # See also
/// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/resources/drive?view=odsp-graph-online
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub id: DriveId,
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

/// The DriveItem resource.
///
/// # See also
/// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/resources/driveitem?view=odsp-graph-online
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveItem {
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
    // deleted: Deleted,
    pub description: Option<String>,
    // pub file_system_info: FileSystemInfo,
    // publication: Option<PublicationFacet>,
    // remote_item: Option<RemoteItem>,
    // search_result: Option<SearchResult>,
    // shared: Shared,
    // sharepoint_ids: SharepointIds,
    pub size: FileSize,
    // web_dav_url: Url,

    // Relationships

    // activities: Vec<ItemActivity>,
    pub children: Option<Vec<DriveItem>>,
    // permissions: Vec<Permission>,
    // thumbnails: Vec<ThumbnailSet>,
    // versions: Vec<DriveItemVersion>,

    // Base item
    pub id: ItemId,
    // created_by: IdentitySet,
    // created_date_time: Timestamp,
    pub e_tag: Tag,
    // last_modified_by: IdentitySet,
    // last_modified_date_time: Timestamp,
    pub name: String,
    // parent_reference: ItemReference,
    pub web_url: Url,

    // Instance annotations
    #[serde(rename = "@microsoft.graph.downloadUrl")]
    pub download_url: Option<Url>,
}
