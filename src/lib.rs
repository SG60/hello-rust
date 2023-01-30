use serde::{Deserialize, Serialize};

pub mod notion_api;
pub mod settings;
pub mod aws;

#[derive(Serialize, Deserialize, Debug)]
pub struct GoogleResponse {
    pub items: Vec<serde_json::Value>,
    pub kind: String,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    pub summary: String,
    #[serde(rename = "timeZone")]
    pub time_zone: String,
    pub updated: String,
}

#[derive(Debug)]
pub struct GoogleToken {
    pub refresh_token: String,
    pub access_token: Option<GoogleAccessToken>,
}

#[derive(Debug)]
pub struct GoogleAccessToken {
    pub access_token: String,
    pub expiry_time: std::time::SystemTime,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoogleRefreshTokenRequestResponse {
    access_token: String, // e.g. "1/fFAasGRNJTz70BzhT3Zg"
    /// in seconds
    expires_in: u64, // e.g. 3920
    scope: String,        // e.g. "https://www.googleapis.com/auth/drive.metadata.readonly"
    token_type: String,   // always "Bearer"
}

impl GoogleToken {
    pub fn new(refresh_token: &str) -> Self {
        Self {
            refresh_token: refresh_token.to_owned(),
            access_token: None,
        }
    }

    /// Refresh the access token
    ///
    /// # Errors
    ///
    /// This function can return an error for several reasons: the request to google fails, the
    /// refresh token is invalid, or the response from google does not match the serde struct.
    ///
    /// TODO: return something different for some of these errors
    pub async fn refresh_token(
        &mut self,
        google_oauth_client_id: &str,
        google_oauth_client_secret: &str,
    ) -> Result<&Self, reqwest::Error> {
        // POST /token HTTP/1.1
        // Host: oauth2.googleapis.com
        // Content-Type: application/x-www-form-urlencoded
        //
        // client_id=your_client_id&
        // client_secret=your_client_secret&
        // refresh_token=refresh_token&
        // grant_type=refresh_token
        let client = reqwest::Client::builder().build()?;
        let params = [
            ("client_id", google_oauth_client_id),
            ("client_secret", google_oauth_client_secret),
            ("refresh_token", &self.refresh_token),
            ("grant_type", "refresh_token"),
        ];
        let response_json = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .and_then(|response| Ok(response.json::<GoogleRefreshTokenRequestResponse>()));

        let response_json = response_json?.await?;

        let expires_in = std::time::Duration::from_secs(response_json.expires_in); // TODO: expiry time
        let expiry_time = std::time::SystemTime::now() + expires_in;

        self.access_token = Some(GoogleAccessToken {
            access_token: response_json.access_token,
            expiry_time,
        });

        Ok(self)
    }

    pub async fn get(
        &mut self,
        google_oauth_client_id: &str,
        google_oauth_client_secret: &str,
    ) -> String {
        let mut expired = false;
        if let Some(ref access_token) = self.access_token {
            if access_token.expiry_time <= std::time::SystemTime::now() {
                expired = true
            }
        } else {
            expired = true
        };

        let refresh_response = if expired {
            println!("Refreshing Google Calendar user access token");
            Some(
                self.refresh_token(google_oauth_client_id, google_oauth_client_secret)
                    .await,
            )
        } else {
            None
        };

        self.access_token
            .as_ref()
            .expect("Access token should exist")
            .access_token
            .to_owned()
    }
}

/// TEMPORARY!
pub fn filter_data_by_hardcoded_user_id() {
    // TEMPORARY! This is a hardcoded user_id string
    let user_id_to_use: &str = "e2TPa0rcNbgDSmPXDA8CtHlOjUN2";

    dbg!(user_id_to_use);
    todo!()
}

pub async fn get_some_data_from_google_calendar(
    bearer_auth_token: &str,
) -> Result<GoogleResponse, reqwest::Error> {
    // client for google requests
    let google_client = reqwest::Client::builder().build()?;

    // Do a request using the google token
    // TODO: make this fetch the correct calendar, rather than the primary one
    let res = google_client
        .get("https://www.googleapis.com/calendar/v3/calendars/primary/events?maxResults=4")
        .bearer_auth(bearer_auth_token)
        .send()
        .await?
        .json::<GoogleResponse>()
        .await?;
    dbg!("from the google response:\n{:#?}", &res.items[0]["summary"]);

    Ok(res)
}
