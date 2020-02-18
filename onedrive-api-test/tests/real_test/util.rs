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

fn login() -> String {
    let env: Env = envy::prefixed("ONEDRIVE_API_TEST_").from_env().unwrap();

    let auth = Authentication::new(
        env.client_id,
        Permission::new_read().write(true).offline_access(true),
        env.redirect_uri,
    );

    auth.login_with_refresh_token(&env.refresh_token, env.client_secret.as_deref())
        .expect("Login failed")
        .token
}

lazy_static! {
    pub static ref TOKEN: String = login();
    pub static ref ONEDRIVE: OneDrive = OneDrive::new(TOKEN.to_owned(), DriveLocation::me());
}

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
