//! DANGER:
//! The integration test requires token and/or refresh_token to send real requests
//! to Microsoft and MAY MODIFY YOUR FILES on OneDrive!
//!
//! Although the test is written to avoid overwriting existing data, you may still
//! take some risks.
//!
//! Login setting file `tests/login_setting.json` is private and is ignored
//! in `.gitignore`, so you need to set up it manually before running this test.
//! The format is specified in `tests/login_setting.json.template`.

mod login_setting;
mod test_drive_client;
