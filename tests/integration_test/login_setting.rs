#![cfg(feature = "reqwest")]
use lazy_static::lazy_static;
use log::{info, warn};
use onedrive_api::*;
use reqwest;
use serde::{Deserialize, Serialize};

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

// NOTE: This supports code auth only.
fn get_token(setting: &mut LoginSetting) -> String {
    let client = reqwest::Client::new();
    let auth = Authentication::new(
        setting.client_id.clone(),
        Permission::new_read().offline_access(true),
        setting.redirect_uri.clone(),
    );

    if let Some(code) = setting.code.take() {
        info!("Login with code...");
        match auth
            .login_with_code(&code, setting.client_secret.as_ref().map(|s| &**s))
            .execute(&client)
        {
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
        let drive = OneDrive::new(token.to_owned(), DriveLocation::me());
        match drive.get_drive().execute(&client) {
            Ok(_) => return token.to_owned(),
            Err(err) => warn!("Failed: {:?}", err),
        }
    }

    if let Some(refresh_token) = &setting.refresh_token {
        info!("Login with refresh token...");
        match auth
            .login_with_refresh_token(&refresh_token, setting.client_secret.as_ref().map(|s| &**s))
            .execute(&client)
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

    panic!("Request code auth: {}", auth.code_auth_url())
}

lazy_static! {
    pub static ref TOKEN: String = {
        use std::fs::{rename, File};
        use std::io::Write;

        env_logger::init();

        let mut setting: LoginSetting = {
            let f = File::open(LOGIN_SETTING_PATH).unwrap();
            serde_json::from_reader(f).expect("Invalid JSON of login setting")
        };
        let client = get_token(&mut setting);
        {
            let mut f = File::create(LOGIN_SETTING_TMP_PATH).unwrap();
            serde_json::to_writer_pretty(&mut f, &setting).expect("Failed to write login setting");
            f.flush().unwrap();
            f.sync_all().unwrap();
        }
        rename(LOGIN_SETTING_TMP_PATH, LOGIN_SETTING_PATH).expect("Failed to update login setting");

        client
    };
}
