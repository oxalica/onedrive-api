//! DANGER:
//! The integration test requires token and/or refresh_token to send real requests to Microsoft and MAY MODIFY YOUR FILES on OneDrive!
//!
//! Although the test is written to avoid overwriting existing data, you may still take some risks.
//!
//! Login setting file `tests/login_setting.json` is private and is ignored in `.gitignore`, so you need to set up it manually before running this test.
//! The format is specified in `tests/login_setting.json.template`.

extern crate onedrive_api; // Hint for RLS

use lazy_static::lazy_static;
use log::{info, warn};
use onedrive_api::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Seek, SeekFrom};

const LOGIN_SETTING_PATH: &str = "tests/login_setting.json";

#[derive(Deserialize, Serialize)]
struct LoginSetting {
    client_id: String,
    client_secret: Option<String>,
    redirect_uri: String,
    refresh_token: Option<String>,
    token: Option<String>,
    code: Option<String>,
}

fn open_setting_file() -> fs::File {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(LOGIN_SETTING_PATH)
        .expect("Login setting file is not found")
}

// NOTE: This supports code auth only.
fn get_token(setting: &mut LoginSetting) -> String {
    let auth_client = AuthClient::new(
        setting.client_id.clone(),
        Scope::ReadWrite {
            shared: false,
            offline: true,
        },
        setting.redirect_uri.clone(),
    );

    if let Some(code) = setting.code.take() {
        info!("Login with code...");
        match auth_client.login_with_code(&code, setting.client_secret.as_ref().map(|s| &**s)) {
            Ok(Token {
                token,
                refresh_token,
                ..
            }) => {
                setting.token = Some(token.clone());
                setting.refresh_token = Some(refresh_token.expect("Cannot get refresh token"));
                return token;
            }
            Err(err) => panic!("Failed to login with code: {:?}", err),
        }
    }

    if let Some(token) = &setting.token {
        info!("Login with token...");
        let client = DriveClient::new(token.to_owned(), DriveLocation::me());
        match client.get_drive() {
            Ok(_) => return token.to_owned(),
            Err(err) => warn!("Failed: {:?}", err),
        }
    }

    if let Some(refresh_token) = &setting.refresh_token {
        info!("Login with refresh token...");
        match auth_client
            .login_with_refresh_token(&refresh_token, setting.client_secret.as_ref().map(|s| &**s))
        {
            Ok(Token {
                token,
                refresh_token,
                ..
            }) => {
                setting.token = Some(token.clone());
                setting.refresh_token = refresh_token;
                return token;
            }
            Err(err) => warn!("Failed: {:?}", err),
        }
    }

    panic!("Request code auth: {}", auth_client.get_code_auth_url())
}

lazy_static! {
    static ref TOKEN: String = {
        env_logger::init();

        let mut f = open_setting_file();
        let mut setting: LoginSetting =
            serde_json::from_reader(&f).expect("Invalid JSON of login setting");
        let client = get_token(&mut setting);
        f.seek(SeekFrom::Start(0)).expect("Failed to seek");
        serde_json::to_writer_pretty(&mut f, &setting).expect("Failed to updating login setting");
        client
    };
}

#[test]
#[ignore]
fn test_get_drive() {
    let client = DriveClient::new(TOKEN.clone(), DriveLocation::me());

    let drive = client.get_drive().expect("Cannot get drive #1");
    let drive_id = drive.id;
    assert!(!drive_id.as_ref().is_empty());

    let drive_from_id = DriveClient::new(TOKEN.clone(), drive_id.clone())
        .get_drive()
        .expect("Cannot get drive #2");
    assert_eq!(drive_from_id.id, drive_id);
}

#[test]
#[ignore]
fn test_file_operations() {
    use self::error::Error;

    let client = DriveClient::new(TOKEN.clone(), DriveLocation::me());

    let folder_item = client
        .create_folder(ItemLocation::root(), FileName::new("test_folder").unwrap())
        .expect("Failed to create folder");

    let file1_path = ItemLocation::from_path("/test_folder/1.txt").unwrap();
    let file1 = client
        .upload_small(file1_path, b"hello")
        .expect("Failed to upload small file");

    let file2_path = ItemLocation::from_path("/test_folder/2.txt").unwrap();
    let file2 = client
        .move_(
            &file1.id,
            &folder_item.id,
            Some(FileName::new("2.txt").unwrap()),
            None,
        )
        .expect("Failed to move file");

    assert!(client
        .get_item(
            file2_path,
            Some(&file2.e_tag), // The file is not changed. Should return `None`.
        )
        .expect("Failed to get file2")
        .is_none());

    let children = client
        .list_children(&folder_item.id, None)
        .expect("Failed to list children")
        .expect("Listing children returns expected None");

    assert_eq!(children.len(), 1);
    assert_eq!(children[0].id, file2.id);
    assert_eq!(children[0].e_tag, file2.e_tag);

    let file3_path = ItemLocation::from_path("/test_folder/3.txt").unwrap();
    let upload_session = client
        .new_upload_session(file3_path, false, None)
        .expect("Failed to create upload session");
    assert!(
        client
            .upload_to_session(&upload_session, b"1234", 0..4, 6)
            .expect("Failed to upload part 1 to session")
            .is_none() // Not done
    );
    let upload_session2 = client
        .get_upload_session(upload_session.get_url())
        .expect("Failed to get upload session");
    let file3 = client
        .upload_to_session(&upload_session2, b"56", 4..6, 6)
        .expect("Failed to upload to session #1")
        .expect("Uploading to session #1 returns expected None");

    // This contains more fields.
    let file3 = client
        .get_item(&file3.id, None)
        .expect("Failed to get file3")
        .expect("Getting file3 returns unexpected None");

    let file3_url = file3.download_url.expect("File3 has no download_url");
    let file3_content = reqwest::get(&file3_url)
        .expect("Failed to GET file3")
        .error_for_status()
        .expect("Error GETing file3")
        .text()
        .expect("Error downloading file3");
    assert_eq!(file3_content, "123456");

    client
        .delete(&folder_item.id, None)
        .expect("Failed to delete");

    let err = client
        .get_item(&folder_item.id, None)
        .expect_err("File should be already deleted");

    assert!(match &err {
        Error::RequestError {
            response: Some(_), ..
        } => true,
        _ => false,
    });
    assert_eq!(err.should_retry(), false);
    assert_eq!(err.status(), Some(reqwest::StatusCode::NOT_FOUND));
}
