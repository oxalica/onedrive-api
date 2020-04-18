use lazy_static::lazy_static;
use onedrive_api::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Env {
    client_id: String,
    client_secret: Option<String>,
    redirect_uri: String,
    refresh_token: String,
}

async fn login() -> String {
    let env: Env = envy::prefixed("ONEDRIVE_API_TEST_").from_env().unwrap();

    let auth = Auth::new(
        env.client_id,
        Permission::new_read().write(true).offline_access(true),
        env.redirect_uri,
    );

    auth.login_with_refresh_token(&env.refresh_token, env.client_secret.as_deref())
        .await
        .expect("Login failed")
        .access_token
}

lazy_static! {
    pub static ref TOKEN: tokio::sync::Mutex<Option<String>> = Default::default();
}

pub async fn get_logined_onedrive() -> OneDrive {
    let mut guard = TOKEN.lock().await;
    let token = match &*guard {
        Some(token) => token.clone(),
        None => {
            let token = login().await;
            *guard = Some(token.clone());
            token
        }
    };
    OneDrive::new(token, DriveLocation::me())
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

pub async fn download(url: &str) -> Vec<u8> {
    reqwest::get(url)
        .await
        .expect("Failed to request for downloading file")
        .bytes()
        .await
        .expect("Failed to download file")
        .to_vec()
}
