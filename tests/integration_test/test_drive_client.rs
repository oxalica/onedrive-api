use onedrive_api::query_option::*;
use onedrive_api::resource::*;
use onedrive_api::*;
use reqwest::StatusCode;

use crate::login_setting::TOKEN;

fn gen_filename() -> &'static FileName {
    use std::sync::atomic::*;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let s = Box::leak(format!("$onedrive_api_tests.{}", id).into_boxed_str());
    FileName::new(s).unwrap()
}

fn rooted_location(name: &FileName) -> ItemLocation<'static> {
    let s = Box::leak(format!("/{}", name.as_str()).into_boxed_str());
    ItemLocation::from_path(s).unwrap()
}

fn try_finally<T>(body: impl FnOnce() -> T, finally: impl FnOnce()) -> T {
    struct Guard<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Guard<F> {
        fn drop(&mut self) {
            (self.0.take().unwrap())();
        }
    }

    let _guard = Guard(Some(finally));
    body()
}

fn download(url: &str) -> Vec<u8> {
    use std::io::Read;

    let mut buf = vec![];
    reqwest::get(url)
        .expect("Failed to request for downloading file")
        .read_to_end(&mut buf)
        .expect("Failed to download file");
    buf
}

/// Max 4 requests.
///
/// # Test
/// - new()
///   - From `me()`.
///   - From drive id.
/// - get_drive()
///   - Success.
/// - get_item()
///   - Success, directory.
///   - Success, directory, with option.
#[test]
#[ignore]
fn test_get_drive() {
    let client = DriveClient::new(TOKEN.clone(), DriveLocation::me());
    let drive1 = client.get_drive().expect("Cannot get drive #1");

    // Default fields.
    let drive1_id = drive1.id.unwrap();

    let drive2 = DriveClient::new(TOKEN.clone(), drive1_id.clone())
        .get_drive_with_option(ObjectOption::new().select(&[&DriveField::id]))
        .expect("Cannot get drive #2");
    assert_eq!(drive1_id, drive2.id.unwrap());

    let root_item = client
        .get_item(ItemLocation::root(), None)
        .expect("Cannot get root item")
        .unwrap();
    assert!(root_item.id.is_some());
    assert!(root_item.e_tag.is_some());

    let root_item2 = client
        .get_item_with_option(
            ItemLocation::root(),
            None,
            ObjectOption::new().select(&[&DriveItemField::e_tag]),
        )
        .expect("Cannot get root item with option")
        .unwrap();
    assert!(root_item2.id.is_none());
    assert!(root_item2.e_tag.is_some());
}

/// Max 8 requests.
///
/// # Test
/// - create_folder()
///   - Success.
///   - Failed (folder exists).
/// - delete()
///   - Success, folder, without tag.
///   - Failed (not exists), folder, without tag.
/// - list_children()
///   - Success (not modified), with tag.
///   - Success, without tag.
///   - Failed (not found), without tag.
#[test]
#[ignore]
fn test_folder() {
    let client = DriveClient::new(TOKEN.clone(), DriveLocation::me());

    let folder1_name = gen_filename();
    let folder2_name = gen_filename();
    let folder2_location = rooted_location(folder2_name);

    let (folder1_id, folder1_e_tag) = {
        let c = client
            .create_folder(ItemLocation::root(), folder1_name)
            .expect("Failed to create folder");
        (c.id.unwrap(), c.e_tag.unwrap())
    };

    try_finally(
        || {
            assert_eq!(
                client
                    .create_folder(ItemLocation::root(), folder1_name)
                    .expect_err("Re-create folder should fail")
                    .status(),
                Some(StatusCode::CONFLICT),
            );

            assert_eq!(
                client
                    .delete(folder2_location, None)
                    .expect_err("Should not delete a file does not exist")
                    .status(),
                Some(StatusCode::NOT_FOUND),
            );

            assert!(
                client
                    .list_children(&folder1_id, Some(&folder1_e_tag))
                    .expect("Failed to list children with tag")
                    .is_none(),
                "Folder should be 'not modified'",
            );

            let folder2 = client
                .create_folder(&folder1_id, folder2_name)
                .expect("Failed to create sub-folder");

            let children = client
                .list_children(&folder1_id, None)
                .expect("Failed to list children")
                .unwrap();

            assert_eq!(children.len(), 1);
            let child = children.into_iter().next().unwrap();
            assert_eq!(child.id, folder2.id);
            assert_eq!(child.name, folder2.name);
            assert_eq!(child.e_tag, folder2.e_tag);
        },
        || {
            client.delete(&folder1_id, None).unwrap();
        },
    );

    assert_eq!(
        client
            .list_children(&folder1_id, None)
            .expect_err("Folder should be already deleted")
            .status(),
        Some(StatusCode::NOT_FOUND),
    );
}

/// Max 5 requests.
///
/// # Test
/// - upload_small()
///   - Success.
/// - get_item()
///   - Success, file, without tag.
/// - move_()
///   - Success, file, without tag.
/// - delete()
///   - Success, file, without tag.
#[test]
#[ignore]
fn test_file_upload_small_and_move() {
    let client = DriveClient::new(TOKEN.clone(), DriveLocation::me());

    const CONTENT: &[u8] = b"hello, world";
    let file1_location = rooted_location(gen_filename());
    let file2_name = gen_filename();

    let file1_id = client
        .upload_small(file1_location, CONTENT)
        .expect("Failed to upload small file")
        .id
        .unwrap();

    let is_moved = std::cell::Cell::new(false);
    let file2_id = try_finally(
        || {
            let file2 = client
                .move_(&file1_id, ItemLocation::root(), Some(file2_name), None)
                .expect("Failed to move file");
            is_moved.set(true);
            file2.id.unwrap()
        },
        || {
            if !is_moved.get() {
                client.delete(&file1_id, None).unwrap();
            }
        },
    );

    try_finally(
        || {
            let file2_download_url = client
                .get_item(&file2_id, None)
                .expect("Failed to get download url of small file")
                .unwrap()
                .download_url
                .unwrap();

            assert_eq!(download(&file2_download_url), CONTENT);
        },
        || {
            client.delete(&file2_id, None).unwrap();
        },
    );
}

/// Max 8 requests.
///
/// # Test
/// - new_upload_session()
///   - Success, no overwriting, without tag.
/// - get_upload_session()
///   - Success.
/// - upload_to_session()
///   - Success (not completed).
///   - Success (completed).
///   - Failed (range error).
/// - get_item()
///   - Success, file, without tag.
/// - delete()
///   - Success, file, without tag,
#[test]
#[ignore]
fn test_file_upload_session() {
    let client = DriveClient::new(TOKEN.clone(), DriveLocation::me());

    type Range = std::ops::Range<usize>;
    const CONTENT: &[u8] = b"12345678";
    const RANGE1: Range = 0..2;
    const RANGE2_ERROR: Range = 6..8;
    const RANGE2: Range = 2..8;

    let upload_session = client
        .new_upload_session(rooted_location(gen_filename()), false, None)
        .expect("Failed to create upload session");

    assert!(
        client
            .upload_to_session(&upload_session, &CONTENT[RANGE1], RANGE1, CONTENT.len())
            .expect("Failed to upload part 1")
            .is_none(),
        "Uploading part 1 should not complete",
    );

    let upload_session = client
        .get_upload_session(upload_session.get_url())
        .expect("Failed to get upload session");
    let next_ranges = upload_session.get_next_expected_ranges();
    assert_eq!(
        next_ranges.len(),
        1,
        "Too many 'next expexted ranges: {:?}",
        next_ranges
    );
    assert_eq!(next_ranges[0].start, RANGE2.start);
    assert!(
        match next_ranges[0].end {
            None => true,
            Some(end) if end == RANGE2.end => true,
            _ => false,
        },
        "Unexpected 'next expected range': {:?}",
        next_ranges[0],
    );

    assert_eq!(
        client
            .upload_to_session(
                &upload_session,
                &CONTENT[RANGE2_ERROR],
                RANGE2_ERROR,
                CONTENT.len(),
            )
            .expect_err("Upload wrong range should fail")
            .status(),
        Some(StatusCode::RANGE_NOT_SATISFIABLE),
    );

    let file3_id = client
        .upload_to_session(&upload_session, &CONTENT[RANGE2], RANGE2, CONTENT.len())
        .expect("Failed to upload part 2")
        .expect("Uploading should be completed")
        .id
        .unwrap();

    try_finally(
        || {
            let file3_download_url = client
                .get_item(&file3_id, None)
                .expect("Failed to get download url of large file")
                .unwrap()
                .download_url
                .unwrap();

            assert_eq!(download(&file3_download_url), CONTENT);
        },
        || {
            client.delete(&file3_id, None).unwrap();
        },
    );
}

/// Max 8 requests.
///
/// # Test
/// - create_folder()
///   - Success.
/// - list_children()
///   - Success, with option, iterator, multi page.
/// - delete()
///   - Success, folder.
#[test]
#[ignore]
fn test_list_children() {
    let client = DriveClient::new(TOKEN.clone(), DriveLocation::me());

    const TOTAL_COUNT: usize = 2;
    const PAGE_SIZE: usize = 1;
    const PAGE1_COUNT: usize = 1;
    const PAGE2_COUNT: usize = 1;

    let folder_id = client
        .create_folder(ItemLocation::root(), gen_filename())
        .expect("Failed to create container folder")
        .id
        .unwrap();
    let folder_location = ItemLocation::from_id(&folder_id);

    try_finally(
        || {
            let mut files: std::collections::HashMap<String, Tag> = (0..TOTAL_COUNT)
                .map(|i| {
                    let name = gen_filename();
                    let item = client
                        .create_folder(folder_location, name)
                        .unwrap_or_else(|e| {
                            panic!("Failed to create child {}/{}: {}", i + 1, TOTAL_COUNT, e)
                        });
                    (name.as_str().to_owned(), item.e_tag.unwrap())
                })
                .collect();

            let mut fetcher: ListChildrenFetcher = client
                .list_children_with_option(
                    folder_location,
                    None,
                    CollectionOption::new()
                        .select(&[&DriveItemField::name, &DriveItemField::e_tag])
                        .page_size(PAGE_SIZE),
                )
                .expect("Failed to list children with option")
                .unwrap();

            let etags_of = |v: &[DriveItem]| -> Vec<Tag> {
                v.iter()
                    .map(|item| item.e_tag.as_ref().cloned().unwrap())
                    .collect()
            };
            let check_page_eq = |url: String, expected: &[DriveItem]| {
                let mut fetcher_ = client.resume_list_children(url);
                let page_ = fetcher_.next().unwrap().expect("Failed to re-get page");
                assert_eq!(etags_of(&page_), etags_of(&expected));
            };

            let url1 = fetcher.get_next_url().unwrap().to_owned();
            let page1 = fetcher.next().unwrap().expect("Failed to fetch page 1");
            assert_eq!(page1.len(), PAGE1_COUNT);
            check_page_eq(url1, &page1);

            let url2 = fetcher.get_next_url().unwrap().to_owned();
            let page2 = fetcher.next().unwrap().expect("Failed to fetch page 2");
            assert_eq!(page2.len(), PAGE2_COUNT);
            check_page_eq(url2, &page2);

            assert!(fetcher.get_next_url().is_none());
            assert!(fetcher.next().is_none());
            assert!(fetcher.next().is_none()); // Check fused.

            std::iter::empty()
                .chain(page1.iter())
                .chain(page2.iter())
                .for_each(|item| {
                    assert!(item.id.is_none()); // Not selected.
                    let expected_tag = files
                        .remove(item.name.as_ref().unwrap())
                        .expect("Unexpected name");
                    assert_eq!(item.e_tag.as_ref().unwrap(), &expected_tag);
                });
            assert!(files.is_empty()); // All matched
        },
        || {
            client
                .delete(folder_location, None)
                .expect("Failed to delete container folder");
        },
    );
}
