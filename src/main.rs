mod aws;
mod settings;
use aws::*;

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

    let sync_records = match get_sync_records(&dynamo_db_client).await {
        Ok(syncs) => syncs,
        Err(e) => return Err(e.into()),
    };

    dbg!(&sync_records);

    println!("Getting one user's sync record");
    let a_user_id = &users[0].user_id;
    let user_sync_record = match get_sync_record(&dynamo_db_client, &a_user_id).await {
        Ok(syncs) => syncs,
        Err(e) => return Err(e.into()),
    };

    dbg!(&user_sync_record);

    Ok(())
}
