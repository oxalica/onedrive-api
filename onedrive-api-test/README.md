## onedrive-api-test

This is a sub-crate to test `onedrive-api` by sending real requests to [Microsoft OneDrive][onedrive].

To run the test, you need to get a token for it.
Here are the steps:

1. Login any of your Microsoft accounts,
   goto [`App Registrition`][app_registrition] on `Microsoft Azure`
   and register a new application.

   - You should choose the proper `Supported account types` to allow it to
     access `personal Microsoft accounts`.
   - In `Redirect URI `, choose `Public client/native (mobile & desktop)` and
     provide URL `https://login.microsoftonline.com/common/oauth2/nativeclient`
     (default URL for native apps).

   After a successful registrition, you got an `Application (client) ID` in UUID format.

2. Create a NEW Microsoft account *only* for test.
   This is highly recommended since the test may corrupt your files in OneDrive.

2. `cd` to the directory containing this README file,
   and run `cargo run -- <client_id_you_get_in_step_1>`.
   It will prompt a browser and let you login to Microsoft.

3. Check and login the **test-only account** in browser, and it will redirect you
   to an blank page with url containing query string `code=...`.

4. Copy **the whole** URL to the console in step 2 and press enter. It will
   retrieve an token for test and save it to `.env` in current directory.

5. `source .env && cargo test`
   It will run tests in OneDrive of your test-only account.

Also, you can revoke tokens granted on https://account.live.com/consent/Manage
 

[onedrive]: https://onedrive.live.com
[app_registrition]: https://portal.azure.com/#blade/Microsoft_AAD_RegisteredApps/ApplicationsListBlade
