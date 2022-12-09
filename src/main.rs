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

    Ok(())
}
