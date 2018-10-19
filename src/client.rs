use std::rc::Rc;
use reqwest::Client;
use super::error::*;
use super::resource::*;

#[derive(Clone, Debug)]
pub enum Scope {
    Read { shared: bool, offline: bool },
    ReadWrite { shared: bool, offline: bool },
}

impl Scope {
    fn to_scope_string(&self) -> &'static str {
        unimplemented!()
    }
}

#[derive(Clone, Debug)]
pub struct LoginParam {
    scope: Scope,
    redirect_uri: String,
}

#[derive(Debug)]
pub struct RefreshToken(String);

pub struct OneDriveClient {
    client: Rc<Client>,
    client_id: String,
    login_param: LoginParam,
}

impl OneDriveClient {
    pub fn new(client_id: String, login_param: LoginParam) -> Self {
        unimplemented!()
    }

    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#token-flow
    pub fn get_token_auth_uri(&self) -> String {
        unimplemented!()
    }
    
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-1-get-an-authorization-code
    pub fn get_code_auth_uri(&self) -> String {
        unimplemented!()
    }

    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/getting-started/graph-oauth?view=odsp-graph-online#step-2-redeem-the-code-for-access-tokens
    pub fn login_with_code(&self) -> Result<(AuthorizedClient, RefreshToken)> {
        unimplemented!()
    }

    pub fn login_with_token(&self, token: &str) -> Result<AuthorizedClient> {
        unimplemented!()
    }

    pub fn login_with_refresh_token(&self, refresh_token: &RefreshToken) -> Result<(AuthorizedClient, RefreshToken)> {
        unimplemented!()
    }

    pub fn refresh_token(&self, client: &mut AuthorizedClient) -> Result<()> {
        unimplemented!()
    }

    pub fn logout(&self, client: &mut AuthorizedClient) -> Result<()> {
        unimplemented!()
    }
}

pub struct AuthorizedClient {
    client: Rc<Client>,
    token: String,
    redirect_uri: String,
}

impl AuthorizedClient {
    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/drive_get?view=odsp-graph-online#get-current-users-onedrive
    pub fn get_current_drive(&self) -> Result<Drive> {
        unimplemented!()
    }

    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_list_children?view=odsp-graph-online
    pub fn list_children<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        match_tag: Option<Tag>,
    ) -> Result<Option<Vec<DriveItem>>> {
        unimplemented!()
    }

    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_get?view=odsp-graph-online
    pub fn get_item<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        match_tag: Option<Tag>,
    ) -> Result<DriveItem> {
        unimplemented!()
    }

    /// https://docs.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_put_content?view=odsp-graph-online
    pub fn upload_small<'a>(
        &self,
        drive: impl Into<DriveLocation<'a>>,
        item: impl Into<ItemLocation<'a>>,
        data: &[u8],
    ) -> Result<DriveItem> {
        unimplemented!()
    }
}
