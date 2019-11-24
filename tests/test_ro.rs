//! Test read-only GET requests using Microsoft sample account through
//! https://developer.microsoft.com/en-us/graph/graph-explorer
//!
//! The tests require network access and are ignored by default.
#![cfg(feature = "reqwest")]
extern crate onedrive_api;
use http::{self, StatusCode};
use insta::assert_debug_snapshot;
use onedrive_api::{option::*, resource::*, *};

/// 3 requests
fn test_get_drive(drive: &OneDrive, client: &impl Client) {
    // #1
    let mut drive1 = drive
        .get_drive()
        .execute(client)
        .expect("Cannot get drive #1");
    // Urls are mutable.
    drive1.web_url = None;
    assert_debug_snapshot!(&drive1);
    assert!(drive1.quota.is_some());
    assert!(drive1.owner.is_some());

    let drive_id = drive1.id.as_ref().expect("drive1 has no id");

    // #2
    let drive2 = OneDrive::new(drive.token().to_owned(), drive_id.clone())
        .get_drive_with_option(ObjectOption::new().select(&[DriveField::id, DriveField::owner]))
        .execute(client)
        .expect("Cannot get drive #2");
    assert_debug_snapshot!(&drive2);
    assert_eq!(
        drive2.id.as_ref().expect("drive2 has no id").as_str(),
        drive_id.as_str()
    );
    assert_eq!(
        drive1.owner.as_ref().unwrap(),
        drive2.owner.as_ref().expect("drive2 has no owner"),
    );
    assert!(drive2.quota.is_none(), "drive2 has unselected `quota`");

    // #3
    assert_eq!(
        OneDrive::new(
            drive.token().to_owned(),
            DriveId::new(format!("{}_inva_lid", drive_id.as_str())),
        )
        .get_drive()
        .execute(client)
        .expect_err("Drive id should be invalid")
        .status_code(),
        // This API returns 400 instead of 404
        Some(StatusCode::BAD_REQUEST),
    );
}

/// 3 requests
fn test_get_item(drive: &OneDrive, client: &impl Client) {
    const ITEM_PATH: &str = "/Notebooks";
    const ITEM_ID: &str = "01BYE5RZ5YOS4CWLFWORAJ4U63SCA3JT5P";

    let item_id = ItemId::new(ITEM_ID.to_owned());
    let item_path = ItemLocation::from_path(ITEM_PATH).unwrap();

    // #1
    let mut item_by_path = drive
        .get_item(item_path)
        .execute(client)
        .expect("Cannot get item by path");
    // Urls are mutable.
    item_by_path.web_url = None;
    assert_debug_snapshot!(&item_by_path);
    assert!(item_by_path.id.is_some(), "Missing `id`");
    assert!(item_by_path.size.is_some(), "Missing `size`");

    // #2
    let mut item_by_id = drive
        .get_item(&item_id)
        .execute(client)
        .expect("Cannot get item by id");
    item_by_id.web_url = None;
    assert_eq!(format!("{:?}", item_by_path), format!("{:?}", item_by_id));

    // #3
    let mut item_custom = drive
        .get_item_with_option(
            &item_id,
            ObjectOption::new()
                .select(&[DriveItemField::id])
                .expand(DriveItemField::children, Some(&["id"])),
        )
        .execute(client)
        .expect("Cannot get item with option")
        .expect("No if-none-match");
    item_custom.web_url = None;
    assert_debug_snapshot!(&item_custom);
    assert_eq!(
        item_custom.id.as_ref(),
        Some(&item_id),
        "`id` should be selected",
    );
    assert!(item_custom.size.is_none(), "`size` should not be selected");

    let children = item_custom.children.expect("`children` should be selected");
    assert!(
        children.iter().all(|item| item.id.is_some()),
        "Child `id` should be selected",
    );
    assert!(
        children.iter().all(|item| item.name.is_none()),
        "Child `name` should not be selected",
    );

    // If-None-Match may be ignored by server
    // let e_tag = item.e_tag.as_ref().expect("item has no e_tag");
    // assert!(
    //     drive
    //         .get_item_with_option(&item_id, ObjectOption::new().if_none_match(&e_tag))
    //         .execute(client)
    //         .expect("Cannot get item by id with if-none-match")
    //         .is_none(),
    //     "Expected to be unchanged",
    // );
}

/// 3 requests
fn test_list_children(drive: &OneDrive, client: &impl Client) {
    fn cmp_drive_item_by_name(lhs: &DriveItem, rhs: &DriveItem) -> std::cmp::Ordering {
        lhs.name
            .as_ref()
            .map(|s| s.as_str())
            .cmp(&rhs.name.as_ref().map(|s| s.as_str()))
    }

    let loc = ItemLocation::from_path("/Notebooks").unwrap();

    // #1
    let mut items = drive
        .list_children(loc)
        .execute(client)
        .expect("Cannot list children");
    // Urls are mutable.
    for item in &mut items {
        item.web_url = None;
        item.download_url = None;
    }
    items.sort_by(cmp_drive_item_by_name);
    assert_debug_snapshot!(&items);
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|c| c.size.is_some()), "Missing `size`");

    // #2
    let mut fetcher = drive
        .list_children_with_option(
            loc,
            CollectionOption::new()
                .select(&[DriveItemField::name, DriveItemField::e_tag])
                .page_size(1),
        )
        .execute(client)
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
        .execute(client)
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
        .execute(client)
        .expect("Cannot fetch page 2")
        .expect("Page 2 should not be None");
    assert_eq!(page2.len(), 1);

    assert!(
        fetcher
            .fetch_next_page()
            .execute(client)
            .expect("Cannot fetch page 3")
            .is_none(),
        "Expected to have only 2 pages",
    );

    let mut items_manual = page1;
    items_manual.extend(page2);
    items_manual.sort_by(cmp_drive_item_by_name);
    assert!(
        items_manual.iter().all(|c| c.size.is_none()),
        "`size` should be not be selected",
    );

    let to_name_etag_pairs = |v: Vec<DriveItem>| -> Vec<(String, Tag)> {
        v.into_iter()
            .map(|item| {
                (
                    item.name.expect("No `name` contained"),
                    item.e_tag.expect("No `e_tag` contained"),
                )
            })
            .collect()
    };
    assert_eq!(to_name_etag_pairs(items), to_name_etag_pairs(items_manual));
}

#[cfg(feature = "reqwest")]
mod reqwest_client {
    use super::*;
    use reqwest;

    #[derive(Debug)]
    pub struct ReqwestClient {
        client: reqwest::Client,
    }

    impl Default for ReqwestClient {
        fn default() -> Self {
            Self {
                client: reqwest::Client::new(),
            }
        }
    }

    impl Client for ReqwestClient {
        fn execute_api(&self, req: http::Request<Vec<u8>>) -> Result<http::Response<Vec<u8>>> {
            let (parts, body) = req.into_parts();
            let url = reqwest::Url::parse_with_params(
                "https://proxy.apisandbox.msdn.microsoft.com/svc",
                &[("url", parts.uri.to_string())],
            )
            .unwrap();
            let mut req = reqwest::Request::new(parts.method, url);
            *req.headers_mut() = parts.headers;
            *req.body_mut() = Some(body.into());

            let mut resp = self.client.execute(req)?;

            let mut b = http::Response::builder();
            b.headers_mut().unwrap().clone_from(resp.headers());
            let mut buf = vec![];
            resp.copy_to(&mut buf)?;
            Ok(b.status(resp.status()).body(buf)?)
        }
    }
}

mod ro {

    macro_rules! test_fns {
        ($($name:ident;)*) => {
            #[cfg(feature = "reqwest")]
            mod reqwest {
                const TOKEN: &str = "{token:https://graph.microsoft.com/}";

                ::lazy_static::lazy_static! {
                    static ref CLIENT: super::super::reqwest_client::ReqwestClient = Default::default();
                }

                $(
                    #[test]
                    #[ignore]
                    fn $name() {
                        use onedrive_api::{OneDrive, DriveLocation};
                        let drive = OneDrive::new(TOKEN.to_owned(), DriveLocation::me());
                        super::super::$name(&drive, &*CLIENT);
                    }
                )*
            }
        };
    }

    test_fns! {
        test_get_drive;
        test_get_item;
        test_list_children;
    }
}
