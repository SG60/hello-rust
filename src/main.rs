mod aws;
mod notion_api;
mod settings;
use aws::*;
use hello_rust::{get_some_data_from_google_calendar, GoogleToken};
use notion_api::get_pages_from_notion_database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Env vars! -----------------------------------
    let settings_map = settings::get_settings();
    println!("{:#?}", settings_map);

    let dynamo_db_client = load_client().await;

    let users = match get_users(&dynamo_db_client).await {
        Ok(users) => users,
        Err(e) => return Err(e.into()),
    };

    dbg!(&users);

    // All the sync records from dynamodb
    let sync_records = match get_sync_records(&dynamo_db_client).await {
        Ok(syncs) => syncs,
        Err(e) => return Err(e.into()),
    };

    dbg!(&sync_records);

    let one_user_record = &users[0];
    println!("Getting one user's sync record");
    let a_user_id = &one_user_record.user_id;
    let user_sync_record = match get_sync_record(&dynamo_db_client, a_user_id).await {
        Ok(syncs) => syncs,
        Err(e) => return Err(e.into()),
    };

    dbg!(&user_sync_record);

    if let Some(notion_data) = &one_user_record.notion_data {
        println!("Getting items from the notion DB");
        let database_id = &user_sync_record[0].notion_database;
        dbg!(database_id);
        dbg!(&notion_data.notion_access_token);
        let notion_events =
            get_pages_from_notion_database(&notion_data.notion_access_token, &database_id).await;

        // dbg!(&notion_events.unwrap().results[0]);
    } else {
        println!("No valid user record");
    };

    if let Some(google_refresh_token) = &one_user_record.google_refresh_token {
        let google_token = GoogleToken::new(google_refresh_token);

        let google_bearer_token = &google_token.access_token.unwrap().access_token;

        dbg!(get_some_data_from_google_calendar(google_bearer_token)
            .await
            .unwrap());

        return Ok(());
    } else {
        return Err("no google refresh token found".into());
    }
}
