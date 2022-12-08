pub async fn load_client() -> aws_sdk_dynamodb::Client {
    let config = aws_config::load_from_env().await;
    aws_sdk_dynamodb::Client::new(&config)
}
