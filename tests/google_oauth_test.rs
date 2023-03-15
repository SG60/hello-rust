use hello_rust_backend::aws::{get_users, load_client};
use hello_rust_backend::settings;
use hello_rust_backend::GoogleToken;

#[tokio::test]
#[ignore]
async fn test_google_oauth_token_refresh() -> Result<(), Box<dyn std::error::Error>> {
    // Env vars! -----------------------------------
    let settings_map = settings::get_settings();
    let settings_map = settings_map.expect("Settings should be set for this test");

    let dynamo_db_client = load_client().await;

    let users = match get_users(&dynamo_db_client).await {
        Ok(users) => users,
        Err(e) => return Err(e.into()),
    };

    let one_user_record = hello_rust_backend::filter_data_by_hardcoded_user_id(&users)
        .expect("should be a record with this user_id");

    if let Some(google_refresh_token) = &one_user_record.google_refresh_token {
        let mut google_token = GoogleToken::new(google_refresh_token);

        _ = google_token
            .refresh_token(
                &settings_map.google_oauth_client_id,
                &settings_map.google_oauth_client_secret,
            )
            .await;

        let access_token = &google_token.access_token.unwrap().access_token;

        assert!(access_token.len() > 10);
        println!("Access token refresh was successful!");

        Ok(())
    } else {
        Err("no google refresh token found".into())
    }
}
