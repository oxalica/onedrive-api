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
extern crate onedrive_api;
use onedrive_api::{option::*, resource::*, *};
use reqwest::StatusCode;
use serde_json::json;

use self::utils::*;

use login::ONEDRIVE;

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
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Randomly initialized counter.
        lazy_static! {
            static ref COUNTER: AtomicUsize = {
                use rand::{rngs::StdRng, Rng, SeedableRng};
                AtomicUsize::new(StdRng::from_entropy().gen())
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
