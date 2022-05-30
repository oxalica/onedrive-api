//! Test read and write APIs by sending real requests to Microsoft Onedrive
//! with token and refresh_token provided. Requires network access.
//!
//! **DANGER:**
//! This may MODIFY YOUR FILES on OneDrive! Although the test is written
//! to avoid overwriting or removing existing data, you may still TAKE SOME RISKS.
//!
//! You should ALWAYS use a new test-only Microsoft account
//! without any important file in its Onedrive.
//!
//! Refresh token should be provided through environment `ONEDRIVE_API_TEST_REFRESH_TOKEN`.
//! Binary target of `onedrive-api-test` is a helper to get it.
#![allow(clippy::redundant_clone)]
use onedrive_api::{option::*, resource::*, *};
use reqwest::StatusCode;
use serde_json::json;

mod util;
use util::*;

use util::get_logined_onedrive as onedrive;

// 3 requests
#[tokio::test]
async fn test_get_drive() {
    let onedrive = onedrive().await;

    // #1
    let drive1 = onedrive.get_drive().await.expect("Cannot get drive #1");
    assert!(drive1.quota.is_some());
    assert!(drive1.owner.is_some());

    let drive_id = drive1.id.as_ref().expect("drive1 has no id");

    // #2
    let drive2 = OneDrive::new(onedrive.access_token(), drive_id.clone())
        .get_drive_with_option(ObjectOption::new().select(&[DriveField::id, DriveField::owner]))
        .await
        .expect("Cannot get drive #2");
    assert_eq!(&drive1.id, &drive2.id); // Checked to be `Some`.
    assert_eq!(&drive1.owner, &drive2.owner); // Checked to be `Some`.
    assert!(drive2.quota.is_none(), "drive2 contains unselected `quota`");

    // #3
    assert_eq!(
        OneDrive::new(
            onedrive.access_token(),
            DriveId(format!("{}_inva_lid", drive_id.as_str())),
        )
        .get_drive()
        .await
        .expect_err("Drive id should be invalid")
        .status_code(),
        // This API returns 400 instead of 404
        Some(StatusCode::BAD_REQUEST),
    );
}

// 3 requests
#[tokio::test]
async fn test_get_item() {
    let onedrive = onedrive().await;

    // #1
    let item_by_path = onedrive
        .get_item(ItemLocation::from_path("/").unwrap())
        .await
        .expect("Cannot get item by path");
    let item_id = item_by_path.id.clone().expect("Missing `id`");

    // #2
    let item_by_id = onedrive
        .get_item(&item_id)
        .await
        .expect("Cannot get item by id");
    assert_eq!(item_by_path.id, item_by_id.id);
    assert_eq!(item_by_path.e_tag, item_by_id.e_tag);
    assert_eq!(item_by_path.name, item_by_id.name);
    // Some fields may change since other tests will modify the content of root dir.

    // #3
    let item_custom = onedrive
        .get_item_with_option(&item_id, ObjectOption::new().select(&[DriveItemField::id]))
        .await
        .expect("Cannot get item with option")
        .expect("No if-none-match");
    assert_eq!(item_custom.id.as_ref(), Some(&item_id), "`id` mismatch",);
    assert!(item_custom.size.is_none(), "`size` should not be selected");

    // `If-None-Match` may be ignored by server.
    // So we don't test it.
}

// 7 requests
#[tokio::test]
async fn test_folder_create_and_list_children() {
    let onedrive = onedrive().await;

    fn to_names(v: Vec<DriveItem>) -> Vec<String> {
        let mut v = v
            .into_iter()
            .map(|item| item.name.expect("Missing `name`"))
            .collect::<Vec<_>>();
        v.sort();
        v
    }

    let container_name = gen_filename();
    let container_loc = rooted_location(container_name);
    let (sub_name1, sub_name2) = (gen_filename(), gen_filename());
    assert!(sub_name1.as_str() < sub_name2.as_str()); // Monotonic
    let items_origin = vec![sub_name1.as_str().to_owned(), sub_name2.as_str().to_owned()];

    // #1
    onedrive
        .create_folder(ItemLocation::root(), container_name)
        .await
        .expect("Cannot create folder");

    onedrive
        .create_folder(container_loc, sub_name1)
        .await
        .expect("Cannot create sub folder 1");
    onedrive
        .create_folder(container_loc, sub_name2)
        .await
        .expect("Cannot create sub folder 2");

    // #2
    let mut fetcher = onedrive
        .list_children_with_option(
            container_loc,
            CollectionOption::new()
                .select(&[DriveItemField::name, DriveItemField::e_tag])
                .page_size(1),
        )
        .await
        .expect("Cannot list children with option")
        .expect("No if-none-match");

    assert!(
        fetcher.next_url().is_none(),
        "`next_url` should be None before page 1",
    );

    // No request for the first page
    let t = std::time::Instant::now();
    let page1 = fetcher
        .fetch_next_page(&onedrive)
        .await
        .expect("Cannot fetch page 1")
        .expect("Page 1 should not be None");
    let elapsed = t.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(1),
        "The first page should be cached",
    );
    assert_eq!(page1.len(), 1);

    assert!(
        fetcher.next_url().is_some(),
        "`next_url` should be Some before page 2",
    );

    // #3
    let page2 = fetcher
        .fetch_next_page(&onedrive)
        .await
        .expect("Cannot fetch page 2")
        .expect("Page 2 should not be None");
    assert_eq!(page2.len(), 1);

    assert!(
        fetcher
            .fetch_next_page(&onedrive)
            .await
            .expect("Cannot fetch page 3")
            .is_none(),
        "Expected to have only 2 pages",
    );

    let mut items_manual = page1;
    items_manual.extend(page2);
    assert!(
        items_manual.iter().all(|c| c.size.is_none()),
        "`size` should be not be selected",
    );
    let items_manual = to_names(items_manual);

    // #4, #5
    let items_shortcut = onedrive
        .list_children(container_loc)
        .await
        .expect("Cannot list children");
    let items_shortcut = to_names(items_shortcut);

    // #6
    let items_expand = onedrive
        .get_item_with_option(
            container_loc,
            ObjectOption::new().expand(DriveItemField::children, Some(&["name"])),
        )
        .await
        .expect("Cannot get item with children")
        .expect("No `If-None-Match`")
        .children
        .expect("Missing `children`");
    let items_expand = to_names(items_expand);

    assert_eq!(items_origin, items_manual);
    assert_eq!(items_origin, items_shortcut);
    assert_eq!(items_origin, items_expand);

    // #7
    onedrive.delete(container_loc).await.unwrap();
}

// 4 requests
#[tokio::test]
async fn test_folder_create_and_delete() {
    let onedrive = onedrive().await;

    let folder_name = gen_filename();
    let folder_loc = rooted_location(folder_name);
    let invalid_path = format!("/{}/invalid", folder_name.as_str());
    let invalid_loc = ItemLocation::from_path(&invalid_path).unwrap();

    // #1
    onedrive
        .create_folder(ItemLocation::root(), folder_name)
        .await
        .expect("Cannot create folder");

    // #2
    assert_eq!(
        onedrive
            .create_folder(ItemLocation::root(), folder_name)
            .await
            .expect_err("Re-create folder should fail by default")
            .status_code(),
        Some(StatusCode::CONFLICT),
    );

    // #3
    assert_eq!(
        onedrive
            .delete(invalid_loc)
            .await
            .expect_err("Should not delete non-existent folder")
            .status_code(),
        Some(StatusCode::NOT_FOUND),
    );

    // #4
    onedrive.delete(folder_loc).await.unwrap();
}

// 4 requests
#[tokio::test]
async fn test_folder_create_and_update() {
    let onedrive = onedrive().await;

    const FAKE_TIME: &str = "2017-01-01T00:00:00Z";

    let folder_name = gen_filename();
    let folder_loc = rooted_location(folder_name);

    fn get_bmtime(item: &DriveItem) -> Option<(&str, &str)> {
        let fs_info = item.file_system_info.as_ref()?.as_object()?;
        Some((
            fs_info.get("createdDateTime")?.as_str()?,
            fs_info.get("lastModifiedDateTime")?.as_str()?,
        ))
    }

    // #1
    let item_before = onedrive
        .create_folder(ItemLocation::root(), folder_name)
        .await
        .expect("Cannot create folder");

    let (btime_before, mtime_before) =
        get_bmtime(&item_before).expect("Invalid file_system_info before update");
    assert_ne!(btime_before, FAKE_TIME);
    assert_ne!(mtime_before, FAKE_TIME);

    // #2
    let mut patch = DriveItem::default();
    patch.file_system_info = Some(Box::new(json!({
        "createdDateTime": FAKE_TIME,
        "lastModifiedDateTime": FAKE_TIME,
    })));
    let item_response = onedrive
        .update_item(folder_loc, &patch)
        .await
        .expect("Cannot update folder metadata");
    assert_eq!(get_bmtime(&item_response), Some((FAKE_TIME, FAKE_TIME)));

    // #3
    let item_after = onedrive
        .get_item(folder_loc)
        .await
        .expect("Cannot get folder before update");
    assert_eq!(get_bmtime(&item_after), Some((FAKE_TIME, FAKE_TIME)));

    // #4
    onedrive.delete(folder_loc).await.unwrap();
}

// 6 requests
#[tokio::test]
async fn test_file_upload_small_and_move() {
    let onedrive = onedrive().await;

    // Different length, since we use `size` to check if replacement is successful.
    const CONTENT1: &[u8] = b"aaa";
    const CONTENT2: &[u8] = b"bbbbbb";

    let file1_loc = rooted_location(gen_filename());
    let file2_name = gen_filename();
    let file2_loc = rooted_location(file2_name);

    // #1
    onedrive
        .upload_small(file1_loc, CONTENT1)
        .await
        .expect("Cannot upload file 1");

    // #2
    onedrive
        .upload_small(file2_loc, CONTENT2)
        .await
        .expect("Cannot upload file 2");

    // #3
    assert_eq!(
        onedrive
            .move_(file1_loc, ItemLocation::root(), Some(file2_name))
            .await
            .expect_err("Should not move with overwrite by default")
            .status_code(),
        Some(StatusCode::CONFLICT),
    );

    // #4
    onedrive
        .move_with_option(
            file1_loc,
            ItemLocation::root(),
            Some(file2_name),
            DriveItemPutOption::new().conflict_behavior(ConflictBehavior::Replace),
        )
        .await
        .expect("Cannot move with overwrite");

    // #5
    assert_eq!(
        onedrive
            .get_item(file2_loc)
            .await
            .expect("Cannot get file2")
            .size
            .expect("Missing `size`"),
        // Content is replaced.
        CONTENT1.len() as i64,
    );

    // #6
    // `file1_loc` is already moved.
    onedrive.delete(file2_loc).await.unwrap();
}

// 5 requests
#[tokio::test]
async fn test_file_upload_small_and_copy() {
    let onedrive = onedrive().await;

    const CONTENT: &[u8] = b"hello, copy";
    const WAIT_TIME: std::time::Duration = std::time::Duration::from_millis(1000);
    const MAX_WAIT_COUNT: usize = 5;

    let name1 = gen_filename();
    let name2 = gen_filename();
    let loc1 = rooted_location(name1);
    let loc2 = rooted_location(name2);

    // #1
    onedrive
        .upload_small(loc1, CONTENT)
        .await
        .expect("Cannot upload file");

    // #2
    let monitor = onedrive
        .copy(loc1, ItemLocation::root(), name2)
        .await
        .expect("Cannot start copy");
    for i in 0.. {
        std::thread::sleep(WAIT_TIME);

        // #3
        match monitor
            .fetch_progress(&onedrive)
            .await
            .expect("Failed to check `copy` progress")
            .status
        {
            CopyStatus::NotStarted | CopyStatus::InProgress => {}
            CopyStatus::Completed => break,
            status => panic!("Unexpected fail of `copy`: {:?}", status),
        }

        if i >= MAX_WAIT_COUNT {
            panic!("Copy timeout");
        }
    }

    // #4, #5
    onedrive.delete(loc2).await.unwrap();
    onedrive.delete(loc1).await.unwrap();
}

// 8 requests
#[tokio::test]
async fn test_file_upload_session() {
    let onedrive = onedrive().await;

    type Range = std::ops::Range<usize>;
    const CONTENT: &[u8] = b"12345678";
    const CONTENT_LEN: u64 = CONTENT.len() as u64;
    const RANGE1: Range = 0..2;
    const RANGE2_ERROR: Range = 6..8;
    const RANGE2: Range = 2..8;

    fn as_range_u64(r: Range) -> std::ops::Range<u64> {
        r.start as u64..r.end as u64
    }

    let item_loc = rooted_location(gen_filename());

    // #1
    let (sess, meta1) = onedrive
        .new_upload_session(item_loc)
        .await
        .expect("Cannot create upload session");

    println!(
        "Upload session will expire at {:?}",
        meta1.expiration_date_time,
    );

    // #2
    assert!(
        sess.upload_part(
            &CONTENT[RANGE1],
            as_range_u64(RANGE1),
            CONTENT_LEN,
            onedrive.client()
        )
        .await
        .expect("Cannot upload part 1")
        .is_none(),
        "Uploading part 1 should not complete",
    );

    // #3
    let meta2 = sess
        .get_meta(onedrive.client())
        .await
        .expect("Cannot get metadata of the upload session");
    let next_ranges = &meta2.next_expected_ranges;
    assert!(
        next_ranges.len() == 1
            && next_ranges[0].start == RANGE2.start as u64
            && next_ranges[0].end.map_or(true, |x| x == RANGE2.end as u64),
        "Invalid `next_expected_ranges`: {:?}",
        next_ranges
    );

    // #4
    assert_eq!(
        sess.upload_part(
            &CONTENT[RANGE2_ERROR],
            as_range_u64(RANGE2_ERROR),
            CONTENT_LEN,
            onedrive.client(),
        )
        .await
        .expect_err("Upload wrong range should fail")
        .status_code(),
        Some(StatusCode::RANGE_NOT_SATISFIABLE),
    );

    // #5
    sess.upload_part(
        &CONTENT[RANGE2],
        as_range_u64(RANGE2),
        CONTENT_LEN,
        onedrive.client(),
    )
    .await
    .expect("Failed to upload part 2")
    .expect("Uploading should be completed");

    // #6
    let download_url = onedrive.get_item_download_url(item_loc).await.unwrap();

    // #7
    assert_eq!(download(&download_url).await, CONTENT);

    // #8
    onedrive.delete(item_loc).await.unwrap();
}

// 8 requests
// This test fetch all changes from root folder, which may contains lots of files and take lots of time.
#[tokio::test]
#[ignore]
async fn test_track_changes() {
    let onedrive = onedrive().await;

    use std::collections::HashSet;

    let container_name = gen_filename();
    let container_loc = rooted_location(container_name);

    // #1
    let container_id = onedrive
        .create_folder(ItemLocation::root(), container_name)
        .await
        .expect("Cannot create container folder")
        .id
        .expect("Missing `id`");

    // #2
    let folder1_id = onedrive
        .create_folder(container_loc, gen_filename())
        .await
        .expect("Failed to create folder1")
        .id
        .expect("Missing `id`");

    // #3
    let folder2_id = onedrive
        .create_folder(container_loc, gen_filename())
        .await
        .expect("Failed to create folder2")
        .id
        .expect("Missing `id`");

    {
        // #4
        let (initial_changes, _) = onedrive
            .track_root_changes_from_initial()
            .await
            .expect("Cannot track initial changes")
            .fetch_all(&onedrive)
            .await
            .expect("Cannot fetch all initial changes");

        // Items may duplicate.
        // See: https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0#remarks
        let ids = initial_changes
            .into_iter()
            .map(|item| item.id.expect("Missing `id`"))
            .collect::<HashSet<ItemId>>();
        // We track changes of root directory, so there may be other files.
        assert!(ids.contains(&container_id));
        assert!(ids.contains(&folder1_id));
        assert!(ids.contains(&folder2_id));
    }

    // #5
    let delta_url = onedrive
        .get_root_latest_delta_url()
        .await
        .expect("Failed to get latest track change delta url");

    // #6
    // Create under folder1
    let folder3_id = onedrive
        .create_folder(&folder1_id, gen_filename())
        .await
        .expect("Failed to create folder3")
        .id
        .expect("Missing `id`");

    // `*`: Update path, from tracing root to every changed file
    // root*
    // |- container*
    //    |- folder1*
    //    |  |- folder3*
    //    |- folder2

    {
        // #7
        let (delta_changes, _) = onedrive
            .track_root_changes_from_delta_url(&delta_url)
            .await
            .expect("Failed to track changes with delta url")
            .fetch_all(&onedrive)
            .await
            .expect("Failed to fetch all changes with delta url");

        let ids = delta_changes
            .into_iter()
            .map(|item| item.id.expect("Missing `id`"))
            .collect::<HashSet<ItemId>>();
        // We track changes of root directory, so there may be other changes.
        assert!(ids.contains(&container_id));
        assert!(ids.contains(&folder1_id));
        assert!(ids.contains(&folder3_id));
        assert!(!ids.contains(&folder2_id)); // This is not updated.
    }

    // #8
    onedrive.delete(container_loc).await.unwrap();
}

#[tokio::test]
async fn test_auth_error() {
    let auth = Auth::new(
        "11111111-2222-3333-4444-555555555555",
        Permission::new_read().offline_access(true),
        "https://login.microsoftonline.com/common/oauth2/nativeclient",
    );

    {
        let err = auth
            .login_with_code("M11111111-2222-3333-4444-555555555555", None)
            .await
            .unwrap_err();
        // Don't know why, but it just replies HTTP `400 Bad Request`.
        assert_eq!(err.status_code(), Some(StatusCode::BAD_REQUEST));
        let err_resp = err.oauth2_error_response().unwrap();
        assert_eq!(err_resp.error, "unauthorized_client"); // Invalid `client_id`.
    }

    {
        let err = auth.login_with_refresh_token("42", None).await.unwrap_err();
        // Don't know why, but it just replies HTTP `400 Bad Request`.
        assert_eq!(err.status_code(), Some(StatusCode::BAD_REQUEST));
        let err_resp = err.oauth2_error_response().unwrap();
        assert_eq!(err_resp.error, "invalid_grant");
    }
}

#[tokio::test]
async fn test_get_drive_error_unauthorized() {
    let onedrive = OneDrive::new("42".to_owned(), DriveLocation::me());
    let err = onedrive.get_drive().await.unwrap_err();
    assert_eq!(err.status_code(), Some(StatusCode::UNAUTHORIZED));
    assert_eq!(
        err.error_response().unwrap().code,
        "InvalidAuthenticationToken",
    );
}
