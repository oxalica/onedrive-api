#![cfg(feature = "reqwest")]
use lazy_static::lazy_static;
use onedrive_api::option::*;
use onedrive_api::resource::*;
use onedrive_api::*;
use reqwest::{self, StatusCode};
use std::collections::{HashMap, HashSet};
use std::iter::{self, FromIterator};

use crate::login_setting::TOKEN;

lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::new();
    static ref DRIVE: OneDrive = OneDrive::new(TOKEN.clone(), DriveLocation::me());
}

fn gen_filename() -> &'static FileName {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Randomly initialized counter.
    lazy_static! {
        static ref COUNTER: AtomicUsize = {
            use rand::{thread_rng, Rng};
            AtomicUsize::new(thread_rng().gen())
        };
    }

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
///   - Success, directory, $select.
#[test]
#[ignore]
fn test_get_drive() {
    let drive1 = DRIVE
        .get_drive()
        .execute(&*CLIENT)
        .expect("Cannot get drive #1");

    // Default fields.
    let drive1_id = drive1.id.unwrap();
    println!("Quota: {}", drive1.quota.unwrap());

    let drive2 = OneDrive::new(TOKEN.clone(), drive1_id.clone())
        .get_drive_with_option(ObjectOption::new().select(&[DriveField::id, DriveField::owner]))
        .execute(&*CLIENT)
        .expect("Cannot get drive #2");
    assert_eq!(drive1_id, drive2.id.unwrap());
    println!("Owner: {}", drive1.owner.unwrap());
    assert!(drive2.quota.is_none()); // Assert not selected.

    let root_item = DRIVE
        .get_item(ItemLocation::root())
        .execute(&*CLIENT)
        .expect("Cannot get root item");
    assert!(root_item.id.is_some());
    assert!(root_item.e_tag.is_some());

    let root_item2 = DRIVE
        .get_item_with_option(
            ItemLocation::root(),
            ObjectOption::new().select(&[DriveItemField::e_tag]),
        )
        .execute(&*CLIENT)
        .expect("Cannot get root item with option")
        .unwrap();
    assert!(root_item2.id.is_none());
    assert!(root_item2.e_tag.is_some());
}

/// Max 9 requests.
///
/// # Test
/// - get_item()
///   - Success, $expand.
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
    let folder1_name = gen_filename();
    let folder2_name = gen_filename();
    let folder2_location = rooted_location(folder2_name);

    let (folder1_id, folder1_e_tag) = {
        let c = DRIVE
            .create_folder(ItemLocation::root(), folder1_name)
            .execute(&*CLIENT)
            .expect("Failed to create folder");
        (c.id.unwrap(), c.e_tag.unwrap())
    };

    try_finally(
        || {
            assert_eq!(
                DRIVE
                    .create_folder(ItemLocation::root(), folder1_name)
                    .execute(&*CLIENT)
                    .expect_err("Re-create folder should fail")
                    .status_code(),
                Some(StatusCode::CONFLICT),
            );

            assert_eq!(
                DRIVE
                    .delete(folder2_location)
                    .execute(&*CLIENT)
                    .expect_err("Should not delete a file does not exist")
                    .status_code(),
                Some(StatusCode::NOT_FOUND),
            );

            assert!(
                DRIVE
                    .list_children_with_option(
                        &folder1_id,
                        CollectionOption::new().if_none_match(&folder1_e_tag),
                    )
                    .execute(&*CLIENT)
                    .expect("Failed to list children with tag")
                    .is_none(),
                "Folder should be 'not modified'",
            );

            let folder2 = DRIVE
                .create_folder(&folder1_id, folder2_name)
                .execute(&*CLIENT)
                .expect("Failed to create sub-folder");
            assert!(folder2.id.is_some());
            assert!(folder2.name.is_some());
            assert!(folder2.e_tag.is_some());

            let children = DRIVE
                .list_children(&folder1_id)
                .execute(&*CLIENT)
                .expect("Failed to list children");

            assert_eq!(children.len(), 1);
            let child = children.into_iter().next().unwrap();
            assert_eq!(child.id, folder2.id);
            assert_eq!(child.name, folder2.name);
            assert_eq!(child.e_tag, folder2.e_tag);

            let item_children = DRIVE
                .get_item_with_option(
                    &folder1_id,
                    ObjectOption::new().expand(DriveItemField::children, Some(&["id"])),
                )
                .execute(&*CLIENT)
                .expect("Failed to use get_item to fetch children")
                .unwrap()
                .children
                .unwrap();
            assert_eq!(item_children.len(), 1);

            let item = item_children.into_iter().next().unwrap();
            assert_eq!(item.id, folder2.id);
            assert!(item.e_tag.is_none());
        },
        || {
            DRIVE.delete(&folder1_id).execute(&*CLIENT).unwrap();
        },
    );

    assert_eq!(
        DRIVE
            .list_children(&folder1_id)
            .execute(&*CLIENT)
            .expect_err("Folder should be already deleted")
            .status_code(),
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
    const CONTENT: &[u8] = b"hello, world";
    let file1_location = rooted_location(gen_filename());
    let file2_name = gen_filename();

    let file1_id = DRIVE
        .upload_small(file1_location, CONTENT)
        .execute(&*CLIENT)
        .expect("Failed to upload small file")
        .id
        .unwrap();

    let is_moved = std::cell::Cell::new(false);
    let file2_id = try_finally(
        || {
            let file2 = DRIVE
                .move_(&file1_id, ItemLocation::root(), Some(file2_name))
                .execute(&*CLIENT)
                .expect("Failed to move file");
            is_moved.set(true);
            file2.id.unwrap()
        },
        || {
            if !is_moved.get() {
                DRIVE.delete(&file1_id).execute(&*CLIENT).unwrap();
            }
        },
    );

    try_finally(
        || {
            let file2_download_url = DRIVE
                .get_item(&file2_id)
                .execute(&*CLIENT)
                .expect("Failed to get download url of small file")
                .download_url
                .unwrap();

            assert_eq!(download(&file2_download_url), CONTENT);
        },
        || {
            DRIVE.delete(&file2_id).execute(&*CLIENT).unwrap();
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
    type Range = std::ops::Range<usize>;
    const CONTENT: &[u8] = b"12345678";
    const RANGE1: Range = 0..2;
    const RANGE2_ERROR: Range = 6..8;
    const RANGE2: Range = 2..8;

    let upload_session = DRIVE
        .new_upload_session(rooted_location(gen_filename()))
        .execute(&*CLIENT)
        .expect("Failed to create upload session");

    println!(
        "Upload session will expire at {:?}",
        upload_session.get_expiration_date_time()
    );

    assert!(
        DRIVE
            .upload_to_session(&upload_session, &CONTENT[RANGE1], RANGE1, CONTENT.len())
            .execute(&*CLIENT)
            .expect("Failed to upload part 1")
            .is_none(),
        "Uploading part 1 should not complete",
    );

    let upload_session = DRIVE
        .get_upload_session(upload_session.get_url())
        .execute(&*CLIENT)
        .expect("Failed to get upload session");
    let next_ranges = upload_session.next_expected_ranges();
    assert_eq!(
        next_ranges.len(),
        1,
        "Too many 'next expexted ranges: {:?}",
        next_ranges
    );
    assert_eq!(next_ranges[0].start, RANGE2.start as u64);
    assert!(
        match next_ranges[0].end {
            None => true,
            Some(end) if end == RANGE2.end as u64 => true,
            _ => false,
        },
        "Unexpected 'next expected range': {:?}",
        next_ranges[0],
    );

    assert_eq!(
        DRIVE
            .upload_to_session(
                &upload_session,
                &CONTENT[RANGE2_ERROR],
                RANGE2_ERROR,
                CONTENT.len(),
            )
            .execute(&*CLIENT)
            .expect_err("Upload wrong range should fail")
            .status_code(),
        Some(StatusCode::RANGE_NOT_SATISFIABLE),
    );

    let file3_id = DRIVE
        .upload_to_session(&upload_session, &CONTENT[RANGE2], RANGE2, CONTENT.len())
        .execute(&*CLIENT)
        .expect("Failed to upload part 2")
        .expect("Uploading should be completed")
        .id
        .unwrap();

    try_finally(
        || {
            let file3_download_url = DRIVE
                .get_item(&file3_id)
                .execute(&*CLIENT)
                .expect("Failed to get download url of large file")
                .download_url
                .unwrap();

            assert_eq!(download(&file3_download_url), CONTENT);
        },
        || {
            DRIVE.delete(&file3_id).execute(&*CLIENT).unwrap();
        },
    );
}

/// Max 7 requests.
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
    const TOTAL_COUNT: usize = 2;
    const PAGE_SIZE: usize = 1;
    const PAGE1_COUNT: usize = 1;
    const PAGE2_COUNT: usize = 1;

    let folder_id = DRIVE
        .create_folder(ItemLocation::root(), gen_filename())
        .execute(&*CLIENT)
        .expect("Failed to create container folder")
        .id
        .unwrap();
    let folder_location = ItemLocation::from_id(&folder_id);

    try_finally(
        || {
            let mut files: HashMap<String, Tag> = (0..TOTAL_COUNT)
                .map(|i| {
                    let name = gen_filename();
                    let item = DRIVE
                        .create_folder(folder_location, name)
                        .execute(&*CLIENT)
                        .unwrap_or_else(|e| {
                            panic!("Failed to create child {}/{}: {}", i + 1, TOTAL_COUNT, e)
                        });
                    (name.as_str().to_owned(), item.e_tag.unwrap())
                })
                .collect();

            let mut fetcher: ListChildrenFetcher = DRIVE
                .list_children_with_option(
                    folder_location,
                    CollectionOption::new()
                        .select(&[DriveItemField::name, DriveItemField::e_tag])
                        .page_size(PAGE_SIZE),
                )
                .execute(&*CLIENT)
                .expect("Failed to list children with option")
                .unwrap();

            let etags_of = |v: &[DriveItem]| -> Vec<Tag> {
                v.iter()
                    .map(|item| item.e_tag.as_ref().cloned().unwrap())
                    .collect()
            };
            let check_page_eq = |url: String, expected: &[DriveItem]| {
                let mut fetcher_ = ListChildrenFetcher::resume_from(&DRIVE.token(), url);
                let page_ = fetcher_
                    .fetch_next_page()
                    .execute(&*CLIENT)
                    .unwrap()
                    .expect("Failed to re-get page");
                assert_eq!(etags_of(&page_), etags_of(&expected));
            };

            // Cannot get next_url for the first page.
            assert!(fetcher.next_url().is_none());
            let page1 = fetcher
                .fetch_next_page()
                .execute(&*CLIENT)
                .unwrap()
                .expect("Failed to fetch page 1");
            dbg!(&page1);
            assert_eq!(page1.len(), PAGE1_COUNT);

            let url2 = fetcher.next_url().unwrap().to_owned();
            let page2 = fetcher
                .fetch_next_page()
                .execute(&*CLIENT)
                .unwrap()
                .expect("Failed to fetch page 2");
            assert_eq!(page2.len(), PAGE2_COUNT);
            check_page_eq(url2, &page2);

            assert!(fetcher.next_url().is_none());
            assert!(fetcher
                .fetch_next_page()
                .execute(&*CLIENT)
                .unwrap()
                .is_none());
            assert!(fetcher.next_url().is_none());
            assert!(fetcher
                .fetch_next_page()
                .execute(&*CLIENT)
                .unwrap()
                .is_none()); // Check fused.

            iter::empty()
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
            DRIVE
                .delete(folder_location)
                .execute(&*CLIENT)
                .expect("Failed to delete container folder");
        },
    );
}

/// Max >=9 requests
///
/// - create_folder()
///   - Success.
/// - track_changes()
///   - Success, with option.
///   - Success.
/// - get_latest_track_change_delta_url()
///   - Success.
/// - delete()
///   - Success, folder.
#[test]
#[ignore]
fn test_track_changes() {
    let container_id = DRIVE
        .create_folder(ItemLocation::root(), gen_filename())
        .execute(&*CLIENT)
        .expect("Failed to create container folder")
        .id
        .unwrap();
    let container_location = ItemLocation::from_id(&container_id);

    try_finally(
        || {
            let folder1_id = DRIVE
                .create_folder(container_location, gen_filename())
                .execute(&*CLIENT)
                .expect("Failed to create folder1")
                .id
                .unwrap();
            let folder2_id = DRIVE
                .create_folder(container_location, gen_filename())
                .execute(&*CLIENT)
                .expect("Failed to create folder2")
                .id
                .unwrap();

            let mut fetcher = DRIVE
                .track_changes_from_initial_with_option(
                    container_location,
                    CollectionOption::new()
                        .select(&[DriveItemField::id])
                        .page_size(1),
                )
                .execute(&*CLIENT)
                .expect("Failed to track initial changes");

            assert!(fetcher.delta_url().is_none());
            assert!(fetcher.next_url().is_none()); // None for the first page.

            let mut delta_ids = HashSet::new();
            let mut i = 0;
            while let Some(page) = fetcher
                .fetch_next_page()
                .execute(&*CLIENT)
                .unwrap_or_else(|e| panic!("Failed to fetch page {}: {}", i + 1, e))
            {
                for item in page {
                    assert!(item.e_tag.is_none()); // Not selected.
                                                   // Items may duplicate.
                                                   // See: https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0#remarks
                    delta_ids.insert(item.id.unwrap());
                }
                i += 1;
            }
            assert!(fetcher.next_url().is_none());
            assert!(fetcher
                .fetch_next_page()
                .execute(&*CLIENT)
                .unwrap()
                .is_none()); // Assert fused.

            // Note that the one of the item is the root folder itself.
            assert_eq!(
                delta_ids,
                HashSet::from_iter(vec![
                    container_id.clone(),
                    folder1_id.clone(),
                    folder2_id.clone()
                ]),
            );

            assert!(fetcher.delta_url().is_some());
            let delta_url = DRIVE
                .get_latest_delta_url(container_location)
                .execute(&*CLIENT)
                .expect("Failed to get latest track change delta url");

            let folder3_id = DRIVE
                .create_folder(ItemLocation::from_id(&folder1_id), gen_filename())
                .execute(&*CLIENT)
                .expect("Failed to create folder3")
                .id
                .unwrap();

            let (v, _) = DRIVE
                .track_changes_from_delta_url(&delta_url)
                .execute(&*CLIENT)
                .and_then(|fetcher| fetcher.fetch_all().execute(&*CLIENT))
                .expect("Failed to track changes with delta url");
            assert_eq!(
                v.into_iter()
                    .map(|item| item.id.unwrap())
                    .collect::<HashSet<_>>(),
                HashSet::from_iter(vec![container_id.clone(), folder1_id, folder3_id]),
            );
        },
        || {
            DRIVE
                .delete(container_location)
                .execute(&*CLIENT)
                .expect("Failed to delete container folder");
        },
    );
}

/// Max 8 requests.
///
/// # Test
/// - upload_small()
///   - Success.
/// - copy()
///   - Success.
/// - move()
///   - Success, with option.
///   - Fail, conflict, with option.
/// - get_item()
///   - Success.
///   - Fail, not fount.
/// - delete()
///   - Success.
#[test]
#[ignore]
fn test_copy_and_conflict_behavior() {
    const FILE_CONTENT: &[u8] = b"1";

    let name1 = gen_filename();
    let name2 = gen_filename();

    let file1_id = DRIVE
        .upload_small(rooted_location(name1), FILE_CONTENT)
        .execute(&*CLIENT)
        .expect("Failed to create file 1")
        .id
        .unwrap();

    try_finally(
        || {
            let monitor = DRIVE
                .copy(&file1_id, ItemLocation::root(), name2)
                .execute(&*CLIENT)
                .expect("Failed to start copy");
            loop {
                match monitor
                    .fetch_progress()
                    .execute(&*CLIENT)
                    .expect("Failed to check `copy` progress")
                    .status
                {
                    CopyStatus::NotStarted | CopyStatus::InProgress => (),
                    CopyStatus::Completed => break,
                    status => panic!("Unexpected fail of `copy`: {:?}", status),
                }
            }

            let file2_id = DRIVE
                .get_item(rooted_location(name2))
                .execute(&*CLIENT)
                .expect("Copy should be done")
                .id
                .unwrap();

            let file2_gone = std::cell::Cell::new(false);

            try_finally(
                || {
                    let move_with = |opt| {
                        DRIVE.move_with_option(
                            &file1_id,
                            ItemLocation::root(),
                            Some(name2),
                            DriveItemPutOption::new().conflict_behavior(opt),
                        )
                    };

                    // Default to be `ConflictBehavior::Fail`
                    assert_eq!(
                        DRIVE
                            .move_(&file1_id, ItemLocation::root(), Some(name2))
                            .execute(&*CLIENT)
                            .expect_err("Move to an existing item should fail")
                            .status_code(),
                        Some(StatusCode::CONFLICT),
                    );

                    let renamed_name2 = move_with(ConflictBehavior::Rename)
                        .execute(&*CLIENT)
                        .expect("Failed to move with rename")
                        .name
                        .unwrap();
                    // Different with both old and new name.
                    assert_ne!(name1.as_str(), renamed_name2);
                    assert_ne!(name2.as_str(), renamed_name2);

                    DRIVE
                        .get_item(&file1_id)
                        .execute(&*CLIENT)
                        .expect("Rename should not replace the target");

                    let replaced_name2 = move_with(ConflictBehavior::Replace)
                        .execute(&*CLIENT)
                        .expect("Failed to move with replace")
                        .name
                        .unwrap();
                    assert_eq!(name2.as_str(), replaced_name2);

                    assert_eq!(
                        DRIVE
                            .get_item(&file2_id)
                            .execute(&*CLIENT)
                            .expect_err("The old file should be replaced")
                            .status_code(),
                        Some(StatusCode::NOT_FOUND),
                    );

                    file2_gone.set(true);
                },
                || {
                    if !file2_gone.get() {
                        DRIVE
                            .delete(&file2_id)
                            .execute(&*CLIENT)
                            .expect("Failed to delete folder2");
                    }
                },
            );
        },
        || {
            DRIVE
                .delete(&file1_id)
                .execute(&*CLIENT)
                .expect("Failed to delete folder 1");
        },
    );
}
