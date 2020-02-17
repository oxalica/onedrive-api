//! Test read-write apis by sending real requests to Microsoft Onedrive
//! with token and/or refresh_token provided in `tests/login_setting.json`.
//!
//! The tests are under feature `test_rw`, require network access,
//! and are ignored by default.
//!
//! **DANGER:**
//! This may MODIFY YOUR FILES on OneDrive!
//! Although the test is written to avoid overwriting existing data,
//! you may still take some risks.
//!
//! Login setting file `tests/login_setting.json` is private and is ignored
//! in `.gitignore`, so you need to set up it manually before running this test.
//! The structure is specified in `tests/login_setting.json.template`.
#![allow(clippy::redundant_clone)]
extern crate onedrive_api;
use onedrive_api::{option::*, resource::*, *};
use reqwest::StatusCode;
use serde_json::json;

use self::utils::*;

use login::ONEDRIVE;

/// 3 requests
#[test]
#[ignore]
fn test_get_drive() {
    // #1
    let drive1 = ONEDRIVE.get_drive().expect("Cannot get drive #1");
    assert!(drive1.quota.is_some());
    assert!(drive1.owner.is_some());

    let drive_id = drive1.id.as_ref().expect("drive1 has no id");

    // #2
    let drive2 = OneDrive::new(ONEDRIVE.token().to_owned(), drive_id.clone())
        .get_drive_with_option(ObjectOption::new().select(&[DriveField::id, DriveField::owner]))
        .expect("Cannot get drive #2");
    assert_eq!(&drive1.id, &drive2.id); // Checked to be `Some`.
    assert_eq!(&drive1.owner, &drive2.owner); // Checked to be `Some`.
    assert!(drive2.quota.is_none(), "drive2 contains unselected `quota`");

    // #3
    assert_eq!(
        OneDrive::new(
            ONEDRIVE.token().to_owned(),
            DriveId::new(format!("{}_inva_lid", drive_id.as_str())),
        )
        .get_drive()
        .expect_err("Drive id should be invalid")
        .status_code(),
        // This API returns 400 instead of 404
        Some(StatusCode::BAD_REQUEST),
    );
}

/// 3 requests
#[test]
#[ignore]
fn test_get_item() {
    // #1
    let mut item_by_path = ONEDRIVE
        .get_item(ItemLocation::from_path("/").unwrap())
        .expect("Cannot get item by path");
    let item_id = item_by_path.id.clone().expect("Missing `id`");

    // #2
    let mut item_by_id = ONEDRIVE.get_item(&item_id).expect("Cannot get item by id");
    // Remove mutable fields.
    item_by_path.web_url = None;
    item_by_path.last_modified_by = None;
    item_by_path.last_modified_date_time = None;
    item_by_id.web_url = None;
    item_by_id.last_modified_by = None;
    item_by_id.last_modified_date_time = None;
    assert_eq!(format!("{:?}", item_by_path), format!("{:?}", item_by_id));

    // #3
    let item_custom = ONEDRIVE
        .get_item_with_option(&item_id, ObjectOption::new().select(&[DriveItemField::id]))
        .expect("Cannot get item with option")
        .expect("No if-none-match");
    assert_eq!(item_custom.id.as_ref(), Some(&item_id), "`id` mismatch",);
    assert!(item_custom.size.is_none(), "`size` should not be selected");

    // `If-None-Match` may be ignored by server.
    // So we don't test it.
}

/// 7 requests
#[test]
#[ignore]
fn test_folder_create_and_list_children() {
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
    ONEDRIVE
        .create_folder(ItemLocation::root(), container_name)
        .expect("Cannot create folder");
    let guard = AutoDelete::new(container_loc);
    ONEDRIVE
        .create_folder(container_loc, sub_name1)
        .expect("Cannot create sub folder 1");
    ONEDRIVE
        .create_folder(container_loc, sub_name2)
        .expect("Cannot create sub folder 2");

    // #2
    let mut fetcher = ONEDRIVE
        .list_children_with_option(
            container_loc,
            CollectionOption::new()
                .select(&[DriveItemField::name, DriveItemField::e_tag])
                .page_size(1),
        )
        .expect("Cannot list children with option")
        .expect("No if-none-match");

    assert!(
        fetcher.next_url().is_none(),
        "`next_url` should be None before page 1",
    );

    // No request for the first page
    let t = std::time::Instant::now();
    let page1 = fetcher
        .fetch_next_page()
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
        .fetch_next_page()
        .expect("Cannot fetch page 2")
        .expect("Page 2 should not be None");
    assert_eq!(page2.len(), 1);

    assert!(
        fetcher
            .fetch_next_page()
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
    let items_shortcut = ONEDRIVE
        .list_children(container_loc)
        .expect("Cannot list children");
    let items_shortcut = to_names(items_shortcut);

    // #6
    let items_expand = ONEDRIVE
        .get_item_with_option(
            container_loc,
            ObjectOption::new().expand(DriveItemField::children, Some(&["name"])),
        )
        .expect("Cannot get item with children")
        .expect("No `If-None-Match`")
        .children
        .expect("Missing `children`");
    let items_expand = to_names(items_expand);

    assert_eq!(items_origin, items_manual);
    assert_eq!(items_origin, items_shortcut);
    assert_eq!(items_origin, items_expand);

    // #7
    drop(guard);
}

/// 4 requests
#[test]
#[ignore]
fn test_folder_create_and_delete() {
    let folder_name = gen_filename();
    let folder_loc = rooted_location(folder_name);
    let invalid_path = format!("/{}/invalid", folder_name.as_str());
    let invalid_loc = ItemLocation::from_path(&invalid_path).unwrap();

    // #1
    ONEDRIVE
        .create_folder(ItemLocation::root(), folder_name)
        .expect("Cannot create folder");
    let guard = AutoDelete::new(folder_loc);

    // #2
    assert_eq!(
        ONEDRIVE
            .create_folder(ItemLocation::root(), folder_name)
            .expect_err("Re-create folder should fail by default")
            .status_code(),
        Some(StatusCode::CONFLICT),
    );

    // #3
    assert_eq!(
        ONEDRIVE
            .delete(invalid_loc)
            .expect_err("Should not delete non-existent folder")
            .status_code(),
        Some(StatusCode::NOT_FOUND),
    );

    // #4
    drop(guard);
}

/// 4 requests
#[test]
#[ignore]
fn test_folder_create_and_update() {
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
    let item_before = ONEDRIVE
        .create_folder(ItemLocation::root(), folder_name)
        .expect("Cannot create folder");
    let guard = AutoDelete::new(folder_loc);

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
    let item_response = ONEDRIVE
        .update_item(folder_loc, &patch)
        .expect("Cannot update folder metadata");
    assert_eq!(get_bmtime(&item_response), Some((FAKE_TIME, FAKE_TIME)));

    // #3
    let item_after = ONEDRIVE
        .get_item(folder_loc)
        .expect("Cannot get folder before update");
    assert_eq!(get_bmtime(&item_after), Some((FAKE_TIME, FAKE_TIME)));

    // #4
    drop(guard);
}

/// 6 requests
#[test]
#[ignore]
fn test_file_upload_small_and_move() {
    // Different length, since we use `size` to check if replacement is successful.
    const CONTENT1: &[u8] = b"aaa";
    const CONTENT2: &[u8] = b"bbbbbb";

    let file1_loc = rooted_location(gen_filename());
    let file2_name = gen_filename();
    let file2_loc = rooted_location(file2_name);

    // #1
    ONEDRIVE
        .upload_small(file1_loc, CONTENT1)
        .expect("Cannot upload file 1");
    let guard1 = AutoDelete::new(file1_loc);

    // #2
    ONEDRIVE
        .upload_small(file2_loc, CONTENT2)
        .expect("Cannot upload file 2");
    let guard2 = AutoDelete::new(file2_loc);

    // #3
    assert_eq!(
        ONEDRIVE
            .move_(file1_loc, ItemLocation::root(), Some(file2_name))
            .expect_err("Should not move with overwrite by default")
            .status_code(),
        Some(StatusCode::CONFLICT),
    );

    // #4
    ONEDRIVE
        .move_with_option(
            file1_loc,
            ItemLocation::root(),
            Some(file2_name),
            DriveItemPutOption::new().conflict_behavior(ConflictBehavior::Replace),
        )
        .expect("Cannot move with overwrite");
    guard1.defuse();

    // #5
    assert_eq!(
        ONEDRIVE
            .get_item(file2_loc)
            .expect("Cannot get file2")
            .size
            .expect("Missing `size`"),
        // Content is replaced.
        CONTENT1.len() as i64,
    );

    // #6
    drop(guard2);
}

/// 5 requests
#[test]
#[ignore]
fn test_file_upload_small_and_copy() {
    const CONTENT: &[u8] = b"hello, copy";
    const WAIT_TIME: std::time::Duration = std::time::Duration::from_millis(1000);
    const MAX_WAIT_COUNT: usize = 5;

    let name1 = gen_filename();
    let name2 = gen_filename();
    let loc1 = rooted_location(name1);
    let loc2 = rooted_location(name2);

    // #1
    ONEDRIVE
        .upload_small(loc1, CONTENT)
        .expect("Cannot upload file");
    let guard1 = AutoDelete::new(loc1);

    // #2
    let monitor = ONEDRIVE
        .copy(loc1, ItemLocation::root(), name2)
        .expect("Cannot start copy");
    for i in 0.. {
        std::thread::sleep(WAIT_TIME);

        // #3
        match monitor
            .fetch_progress()
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
    let guard2 = AutoDelete::new(loc2);

    // #4
    drop(guard2);
    // #5
    drop(guard1);
}

/// 8 requests
#[test]
#[ignore]
fn test_file_upload_session() {
    type Range = std::ops::Range<usize>;
    const CONTENT: &[u8] = b"12345678";
    const RANGE1: Range = 0..2;
    const RANGE2_ERROR: Range = 6..8;
    const RANGE2: Range = 2..8;

    let item_loc = rooted_location(gen_filename());

    // #1
    let sess = ONEDRIVE
        .new_upload_session(item_loc)
        .expect("Cannot create upload session");

    println!(
        "Upload session will expire at {:?}",
        sess.expiration_date_time(),
    );
    let guard = AutoDelete::new_sess(&sess);

    // #2
    assert!(
        ONEDRIVE
            .upload_to_session(&sess, &CONTENT[RANGE1], RANGE1, CONTENT.len())
            .expect("Cannot upload part 1")
            .is_none(),
        "Uploading part 1 should not complete",
    );

    // #3
    let sess = ONEDRIVE
        .get_upload_session(sess.upload_url())
        .expect("Cannot re-get upload session");
    let next_ranges = sess.next_expected_ranges();
    assert!(
        next_ranges.len() == 1
            && next_ranges[0].start == RANGE2.start as u64
            && next_ranges[0].end.map_or(true, |x| x == RANGE2.end as u64),
        "Invalid `next_expected_ranges`: {:?}",
        next_ranges
    );

    // #4
    assert_eq!(
        ONEDRIVE
            .upload_to_session(&sess, &CONTENT[RANGE2_ERROR], RANGE2_ERROR, CONTENT.len(),)
            .expect_err("Upload wrong range should fail")
            .status_code(),
        Some(StatusCode::RANGE_NOT_SATISFIABLE),
    );

    // #5
    ONEDRIVE
        .upload_to_session(&sess, &CONTENT[RANGE2], RANGE2, CONTENT.len())
        .expect("Failed to upload part 2")
        .expect("Uploading should be completed");
    guard.defuse();
    let guard = AutoDelete::new(item_loc);

    // #6
    let download_url = ONEDRIVE
        .get_item(item_loc)
        .expect("Cannot get download url")
        .download_url
        .expect("Cannot get `download_url`");

    // #7
    assert_eq!(download(&download_url), CONTENT);

    // #8
    drop(guard);
}

/// 8 requests
#[test]
#[ignore]
fn test_track_changes() {
    use std::{collections::HashSet, iter::FromIterator};

    let container_name = gen_filename();
    let container_loc = rooted_location(container_name);

    // #1
    let container_id = ONEDRIVE
        .create_folder(ItemLocation::root(), container_name)
        .expect("Cannot create container folder")
        .id
        .expect("Missing `id`");
    let guard = AutoDelete::new(container_loc);

    // #2
    let folder1_id = ONEDRIVE
        .create_folder(container_loc, gen_filename())
        .expect("Failed to create folder1")
        .id
        .expect("Missing `id`");

    // #3
    let folder2_id = ONEDRIVE
        .create_folder(container_loc, gen_filename())
        .expect("Failed to create folder2")
        .id
        .expect("Missing `id`");

    // #4
    let (initial_changes, _) = ONEDRIVE
        .track_changes_from_initial(container_loc)
        .expect("Cannot track initial changes")
        .fetch_all()
        .expect("Cannot fetch all initial changes");

    // Items may duplicate.
    // See: https://docs.microsoft.com/en-us/graph/api/driveitem-delta?view=graph-rest-1.0#remarks
    assert_eq!(
        initial_changes
            .into_iter()
            .map(|item| { item.id.expect("Missing `id`") })
            .collect::<HashSet<ItemId>>(),
        // The root folder itself is contained.
        HashSet::from_iter(vec![
            container_id.clone(),
            folder1_id.clone(),
            folder2_id.clone(),
        ]),
    );

    // #5
    let delta_url = ONEDRIVE
        .get_latest_delta_url(container_loc)
        .expect("Failed to get latest track change delta url");

    // #6
    // Create under folder1
    let folder3_id = ONEDRIVE
        .create_folder(&folder1_id, gen_filename())
        .expect("Failed to create folder3")
        .id
        .expect("Missing `id`");

    // (`*` for update path)
    // root*
    // |- container*
    //    |- folder1*
    //    |  |- folder3*
    //    |- folder2

    // #7
    let (delta_changes, _) = ONEDRIVE
        .track_changes_from_delta_url(&delta_url)
        .expect("Failed to track changes with delta url")
        .fetch_all()
        .expect("Failed to fetch all changes with delta url");
    assert_eq!(
        delta_changes
            .into_iter()
            .map(|item| item.id.expect("Missing `id`"))
            .collect::<HashSet<ItemId>>(),
        // The path from root to every changed file
        HashSet::from_iter(vec![container_id.clone(), folder1_id, folder3_id]),
    );

    // #8
    drop(guard);
}

mod utils {
    use super::ONEDRIVE;
    use lazy_static::lazy_static;
    use onedrive_api::*;

    pub struct AutoDelete<'a> {
        item: Option<ItemLocation<'a>>,
        sess: Option<&'a UploadSession>,
    }

    impl<'a> AutoDelete<'a> {
        pub fn new(item: impl Into<ItemLocation<'a>>) -> Self {
            Self {
                item: Some(item.into()),
                sess: None,
            }
        }

        pub fn new_sess(sess: &'a UploadSession) -> Self {
            Self {
                item: None,
                sess: Some(sess),
            }
        }

        pub fn defuse(self) {
            // FIXME: May leak.
            std::mem::forget(self);
        }
    }

    impl Drop for AutoDelete<'_> {
        fn drop(&mut self) {
            if let Some(item) = self.item {
                match ONEDRIVE.delete(item) {
                    Err(e) if !std::thread::panicking() => {
                        panic!("Cannot delete item {:?}: {}", self.item, e);
                    }
                    _ => {}
                }
            }
            if let Some(sess) = self.sess {
                match ONEDRIVE.delete_upload_session(sess) {
                    Err(e) if !std::thread::panicking() => {
                        panic!("Cannot delete upload session {:?}: {}", sess, e);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn gen_filename() -> &'static FileName {
        use std::sync::atomic::{AtomicU64, Ordering};

        // Randomly initialized counter.
        lazy_static! {
            static ref COUNTER: AtomicU64 = {
                use rand::{rngs::StdRng, Rng, SeedableRng};
                // Avoid overflow to keep it monotonic.
                AtomicU64::new(u64::from(StdRng::from_entropy().gen::<u32>()))
            };
        }

        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let s = Box::leak(format!("$onedrive_api_tests.{}", id).into_boxed_str());
        FileName::new(s).unwrap()
    }

    pub fn rooted_location(name: &FileName) -> ItemLocation<'static> {
        let s = Box::leak(format!("/{}", name.as_str()).into_boxed_str());
        ItemLocation::from_path(s).unwrap()
    }

    pub fn download(url: &str) -> Vec<u8> {
        let mut buf = vec![];
        reqwest::get(url)
            .expect("Failed to request for downloading file")
            .copy_to(&mut buf)
            .expect("Failed to download file");
        buf
    }
}

mod login {
    use lazy_static::lazy_static;
    use onedrive_api::{DriveLocation, OneDrive};
    use serde::{Deserialize, Serialize};
    use serde_json;
    use std::fs;

    const LOGIN_SETTING_PATH: &str = "tests/login_setting.json";
    const LOGIN_SETTING_TMP_PATH: &str = "tests/login_setting.json.tmp";

    #[derive(Deserialize, Serialize)]
    struct LoginSetting {
        client_id: String,
        client_secret: Option<String>,
        redirect_uri: String,
        refresh_token: Option<String>,
        token: Option<String>,
        code: Option<String>,
    }

    // Support code auth only currently.
    fn check_token(setting: &mut LoginSetting) -> String {
        use onedrive_api::{Authentication, Permission};

        let auth = Authentication::new(
            setting.client_id.clone(),
            Permission::new_read().write(true).offline_access(true),
            setting.redirect_uri.clone(),
        );

        if let Some(code) = setting.code.take() {
            println!("Login with code...");
            let tok = auth
                .login_with_code(&code, setting.client_secret.as_ref().map(|s| &**s))
                .expect("Failed to login with code");
            setting.token = Some(tok.token.clone());
            setting.refresh_token = Some(tok.refresh_token.expect("Cannot get refresh token"));
            return tok.token;
        }

        if let Some(token) = &setting.token {
            println!("Try get_drive with given token...");
            match OneDrive::new(token.to_owned(), DriveLocation::me()).get_drive() {
                Ok(_) => return token.to_owned(),
                Err(err) => println!("`get_drive` failed: {:?}", err),
            }
        }

        if let Some(refresh_token) = &setting.refresh_token {
            println!("Login with refresh token...");
            match auth.login_with_refresh_token(
                &refresh_token,
                setting.client_secret.as_ref().map(|s| &**s),
            ) {
                Ok(tok) => {
                    setting.token = Some(tok.token.clone());
                    setting.refresh_token = tok.refresh_token;
                    return tok.token;
                }
                Err(err) => println!("Token refresh failed: {:?}", err),
            }
        }

        panic!("Request code auth: {}", auth.code_auth_url())
    }

    fn init_token() -> String {
        // This file is set up by user.
        let buf = fs::read(LOGIN_SETTING_PATH)
            .map_err(|e| format!("Cannot open setting file '{}': {}", LOGIN_SETTING_PATH, e))
            .unwrap();
        let mut setting: LoginSetting = serde_json::from_slice(&buf)
            .map_err(|e| format!("Invalid setting file '{}': {}", LOGIN_SETTING_PATH, e))
            .unwrap();

        let tok = check_token(&mut setting);

        let buf = serde_json::to_vec_pretty(&setting).expect("Cannot serialize setting");
        fs::write(LOGIN_SETTING_TMP_PATH, &buf)
            .map_err(|e| {
                format!(
                    "Cannot write to temp file '{}': {}",
                    LOGIN_SETTING_TMP_PATH, e,
                )
            })
            .unwrap();
        fs::rename(LOGIN_SETTING_TMP_PATH, LOGIN_SETTING_PATH)
            .map_err(|e| {
                format!(
                    "Cannot rename temp file '{}' to '{}': {}",
                    LOGIN_SETTING_TMP_PATH, LOGIN_SETTING_PATH, e,
                )
            })
            .unwrap();

        tok
    }

    lazy_static! {
        pub static ref TOKEN: String = init_token();
        pub static ref ONEDRIVE: OneDrive = OneDrive::new(TOKEN.to_owned(), DriveLocation::me());
    }
}
