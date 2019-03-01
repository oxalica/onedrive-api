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

/// Max 2 requests.
///
/// # Test
/// - new()
///   - From `me()`.
///   - From drive id.
#[test]
#[ignore]
fn test_get_drive() {
    let drive_id1 = DriveClient::new(TOKEN.clone(), DriveLocation::me())
        .get_drive()
        .expect("Cannot get drive #1")
        .id;

    let drive_id2 = DriveClient::new(TOKEN.clone(), drive_id1.clone())
        .get_drive()
        .expect("Cannot get drive #2")
        .id;

    assert_eq!(drive_id1, drive_id2);
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

    let folder1 = client
        .create_folder(ItemLocation::root(), folder1_name)
        .expect("Failed to create folder");

    try_finally(
        || {
            let ret = client.create_folder(ItemLocation::root(), folder1_name);
            assert!(ret.is_err(), "Re-create folder should fail");
            assert_eq!(ret.unwrap_err().status(), Some(StatusCode::CONFLICT));

            let ret = client.delete(folder2_location, None);
            assert!(ret.is_err(), "Should not delete a file does not exist");
            assert_eq!(ret.unwrap_err().status(), Some(StatusCode::NOT_FOUND));

            assert!(
                client
                    .list_children(&folder1.id, Some(&folder1.e_tag))
                    .expect("Failed to list children with tag")
                    .is_none(),
                "Folder should be 'not modified'",
            );

            let folder2 = client
                .create_folder(&folder1.id, folder2_name)
                .expect("Failed to create sub-folder");

            let children = client
                .list_children(&folder1.id, None)
                .expect("Failed to list children")
                .unwrap();

            assert_eq!(children.len(), 1);
            let child = children.into_iter().next().unwrap();
            assert_eq!(child.id, folder2.id);
            assert_eq!(child.name, folder2.name);
            assert_eq!(child.e_tag, folder2.e_tag);
        },
        || {
            client.delete(&folder1.id, None).unwrap();
        },
    );

    let ret = client.list_children(&folder1.id, None);
    assert!(ret.is_err(), "Folder should be already deleted");
    assert_eq!(ret.unwrap_err().status(), Some(StatusCode::NOT_FOUND));
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
        .id;

    let is_moved = std::cell::Cell::new(false);
    let file2_id = try_finally(
        || {
            let file2 = client
                .move_(&file1_id, ItemLocation::root(), Some(file2_name), None)
                .expect("Failed to move file");
            is_moved.set(true);
            file2.id
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

    let ret = client.upload_to_session(
        &upload_session,
        &CONTENT[RANGE2_ERROR],
        RANGE2_ERROR,
        CONTENT.len(),
    );
    assert!(ret.is_err(), "Upload wrong range should fail");
    assert_eq!(
        ret.unwrap_err().status(),
        Some(StatusCode::RANGE_NOT_SATISFIABLE)
    );

    let file3_id = client
        .upload_to_session(&upload_session, &CONTENT[RANGE2], RANGE2, CONTENT.len())
        .expect("Failed to upload part 2")
        .expect("Uploading should be completed")
        .id;

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
