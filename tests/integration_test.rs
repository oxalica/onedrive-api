//! DANGER:
//! The integration test requires token and/or refresh_token to send real requests to Microsoft and MAY MODIFY YOUR FILES on OneDrive!
//!
//! Although the test is written to avoid overwriting existing data, you may still take some risks.
//!
//! Login setting file `tests/login_setting.json` is private and is ignored in `.gitignore`, so you need to set up it manually before running this test.
//! The format is specified in `tests/login_setting.json.template`.

extern crate onedrive_api; // Hint for RLS

use lazy_static::lazy_static;
use log::{error, info};
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
fn login_client(setting: &mut LoginSetting) -> Client {
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
            Ok(client) => {
                setting.token = Some(client.get_token().to_owned());
                setting.refresh_token = Some(client.get_refresh_token().unwrap().to_owned());
                return client;
            }
            Err(err) => panic!("Failed to login with code: {:?}", err),
        }
    }

    if let Some(token) = &setting.token {
        info!("Login with token...");
        let client = Client::new(token.to_owned(), None);
        match client.get_drive(DriveLocation::CurrentDrive) {
            Ok(_) => return client,
            Err(err) => error!("Failed: {:?}", err),
        }
    }

    if let Some(refresh_token) = &setting.refresh_token {
        info!("Login with refresh token...");
        match auth_client
            .login_with_refresh_token(&refresh_token, setting.client_secret.as_ref().map(|s| &**s))
        {
            Ok(client) => {
                setting.token = Some(client.get_token().to_owned());
                setting.refresh_token = client.get_refresh_token().map(str::to_owned);
                return client;
            }
            Err(err) => error!("Failed: {:?}", err),
        }
    }

    panic!("Request code auth: {}", auth_client.get_code_auth_url())
}

lazy_static! {
    static ref THE_CLIENT: Client = {
        env_logger::init();

        let mut f = open_setting_file();
        let mut setting: LoginSetting =
            serde_json::from_reader(&f).expect("Invalid JSON of login setting");
        let client = login_client(&mut setting);
        f.seek(SeekFrom::Start(0)).expect("Failed to seek");
        serde_json::to_writer_pretty(&mut f, &setting).expect("Failed to updating login setting");
        client
    };
}

#[test]
fn test_get_drive() {
    let client: &Client = &THE_CLIENT;

    let drive = client.get_drive(DriveLocation::CurrentDrive).unwrap();
    let drive_id = drive.id;
    assert!(!drive_id.as_ref().is_empty());

    let drive_from_id = client.get_drive(DriveLocation::DriveId(&drive_id)).unwrap();
    assert_eq!(drive_from_id.id, drive_id);
}
