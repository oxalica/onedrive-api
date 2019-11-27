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
