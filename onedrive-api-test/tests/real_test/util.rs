use onedrive_api::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Env {
    client_id: String,
    redirect_uri: String,
    refresh_token: String,
}

pub static TOKEN: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

pub async fn get_logined_onedrive() -> OneDrive {
    let token = TOKEN
        .get_or_init(|| async {
            let env: Env = envy::prefixed("ONEDRIVE_API_TEST_").from_env().unwrap();
            let auth = Auth::new(
                env.client_id,
                Permission::new_read().write(true).offline_access(true),
                env.redirect_uri,
                Tenant::Consumers,
            );
            auth.login_with_refresh_token(&env.refresh_token, &ClientCredential::None)
                .await
                .expect("Login failed")
                .access_token
        })
        .await;
    OneDrive::new(token.clone(), DriveLocation::me())
}

pub fn gen_filename() -> &'static FileName {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;

    // Randomly initialized counter.
    static COUNTER: OnceLock<AtomicU64> = OnceLock::new();
    let id = COUNTER
        // Avoid overflow to keep it monotonic.
        .get_or_init(|| AtomicU64::new(rand::random::<u32>().into()))
        .fetch_add(1, Ordering::Relaxed);
    let s = Box::leak(format!("$onedrive_api_tests.{id}").into_boxed_str());
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
