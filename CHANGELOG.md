# v0.6.1

## Features
- Default features of `reqwest` can be disabled by `default-features = false`
  to allow switching to non-default tls implementation.
- Enable gzip by default.
- New API: `get_item_download_url[_with_option]`
- New variant of `ItemLocation`: locating an child by name under an `ItemId`.

## Fixes
- `options::*` are now `Send + Sync`

# v0.6.0

## Huge Breaking Changes
- Sweet `reqwest` again. Drop `api::*` indroduced in `0.5.*`.
- Everything is `async` now.
- Beta APIs are now under feature gate `beta`.
- `onedrive_api::auth`
  - `async`!
  - `Authentication` -> `Auth`, and remove methods of token auth flow.
  - `Token` -> `TokenResponse`, and include all fields in Microsoft Reference.
  - `token` -> `access_token` to follow the reference.
- `onedrive_api::error`
  - Switch to `thiserror`.
  - `ErrorObject` -> `ErrorResponse`, and fix types.
  - Now also handle OAuth2 error response.
- String wrappers `onedrive_api::{DriveId, ItemId, Tag}`
  - Now have fields public.
- `onedrive_api::OneDrive`
  - `async`!
  - `UploadSession` now stores `file_size`.
    Methods `upload_to_session` and `delete_upload_session` are moved to `UploadSession`.
  - `*Fetcher` are now pure data struct without references to `OneDrive`.
    This makes it easy to store them with `OneDrive` without worries about self-references.
  - Other tiny fix-ups and renames.

## Features
- `async`!
- Refactor tests and switch to GitHub Actions as CI.

## Fixes
- Shrink dependencies.

# v0.5.2

## Features
- Add new api `OneDrive::update_item[_with_option]`
- Derive `Serialize` and `Default` for resource objects in `onedrive_api::resource`

## Fixes
- Tests

# v0.5.1
## Fixes
- Tests

# v0.5.0
## Huge Breaking Changes
- Refactor all APIs with new `Api` and `Client` interfaces and strip dependency to `reqwest`.
  See docs for more details.
- `Error::{should_retry,url}` are removed.
- Rename `AuthClient` to `Authentication`, and `DriveClient` to `OneDrive`.
- Rename `UploadSession::{get_url,get_next_expected_ranges,get_expiration_date_time}` to `{upload_url,next_expected_ranges,expiration_date_time}`.
- Rename `ListChildrenFetcher::get_next_url` to `next_url`
- Rename `TrackChangeFetcher::{get_next_url,get_delta_url` to `{next_url,delta_url}`
- Rename `ListChildrenFetcher` and `TrackChangeFetcher` are no longer `Iterator`.
  See docs for more details.

## Features
- Refactor and add more tests.
- Support custom HTTP backend.

## Fixes
- Request changes of beta api `CopyProgressMonitor::fetch_progress`
- Documentations

# v0.4.0
## Breaking Changes
- Renane mod `query_option` to `option`.
- Move `if-match` and `if-none-match` from parameter to `option`
  to simplify simple API (without `_with_option`).

## Features
- Support `conflict_behavior` in related `with_option` API.
- Support `expiration_date_time` field in `UploadSession`.
- Support tracking asynchronous `copy` operation through `CopyProgressMonitor`.

## Fixes
- Fix and add more documentations.
- Maintain mod structure.

# v0.3.0
## Features
- Add all fields available of `resource::{Drive, DriveItem}` in Microsoft Graph Documentation (See documentations of them).

## Breaking Changes
- Refact `query_option::{Object, Collection}Option` and change parameter types of relative `DriveClient::*_with_option` methods.
- Remove `resource::{Deleted, ItemReference}`, which are not necessary for using this crate.
  If you need more detail from these fields, just manually destruct the `serde_json::Value` fields of `resource::{Drive, DriveItem}`.

# v0.2.1
## Fixes
- Fix documentations and add examples.

# v0.2.0
Initial release.
