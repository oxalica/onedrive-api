# v0.8.0

## Breaking Changes
- Split metadata out from `UploadSession` and make it works without `OneDrive` instance since it doesn't
  require token.

  - New struct `UploadSessionMeta`
  - `file_size` is separated from `UploadSession` now and is required in each `UploadSession::upload_part`
    call.
  - `OneDrive::get_upload_session` is moved to `UploadSession::get_meta`.

## Features
- Add method `OneDrive::client` to get the `Client` used to constuct the instance.
- Expose constants `OneDrive::UPLOAD_SMALL_MAX_SIZE` and `UploadSession::MAX_PART_SIZE`.

## Fixes
- Fix deserialization error of `OneDrive::upload_small` when uploading empty data.

# v0.7.0

## Breaking Changes
- Limit tracking API to root folders only since they are undocumented and doesn't work in some cases.

  These API are affected:
  - `track_changes_from_{initial,delta_url}{,_with_option}` -> `track_root_changes*`
  - `get_latest_delta_url{,_with_option}` -> `get_root_latest_delta_url*`

  The new API works only for root folders, and the previous `folder` parameter is removed.

## Others
- Update dependencies to `tokio` 1.0 ecosystem.


# v0.6.3

## Fixes
- Revert `track_changes_from_delta_url_with_option` since it will cause duplicated query parameters.
  Instead, we introduced `get_latest_delta_url_with_option` for setting options at beginning.

# v0.6.2 (Yanked)

## Features
- Add missing `track_changes_from_delta_url_with_option` for customizing `track_changes_from_delta_url`.
- Add method `raw_name` for field descriptor enums to get raw camelCased name used in underlying requests.
- Add getter `client_id/permission/redirect_uri` for `Auth`.

## Others
- Bump dependencies.
- Use new rustc features to simplify codes.

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
