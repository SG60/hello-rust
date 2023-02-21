use std::time::Duration;
use tokio::signal;
use tokio::sync::watch;

mod aws;
mod notion_api;
mod settings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let (send, mut recv): (Sender<()>, _) = channel(1);
    let (tx, rx) = watch::channel(());

    // Env vars! -----------------------------------
    let mut retry_wait_seconds = 1;
    let settings_map = loop {
        let settings_map = settings::get_settings();
        match settings_map {
            Err(error) => {
                println!("Error obtaining settings");
                println!("{:#?}", error);
                tokio::time::sleep(Duration::from_secs(retry_wait_seconds)).await;
                if retry_wait_seconds < 300 {
                    retry_wait_seconds += retry_wait_seconds
                };
            }
            Ok(settings_map) => break settings_map,
        }
    };
    println!("Settings successfully obtained.");
    println!("{:#?}", settings_map);

    async fn some_operation(message: &str, duration: Duration, receiver: watch::Receiver<()>) {
        loop {
            tokio::time::sleep(duration).await;
            println!("{}", message);
            if receiver.has_changed().unwrap_or(true) {
                break;
            };
        }
        println!("Task shutting down. ({})", message);

        // sender goes out of scope ...
    }

    let _op1 = tokio::spawn(some_operation(
        "Hello World!",
        Duration::from_secs(3),
        rx.clone(),
    ));

    let _op2 = tokio::spawn(some_operation(
        "hello world from a shorter loop!",
        Duration::from_secs(7),
        rx.clone(),
    ));

    match signal::ctrl_c().await {
        Ok(()) => {
            println!("Goodbye!");
        }
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            // we also shut down in case of error
        }
    }
    // send shutdown signal to application and wait
    tx.send(())?;

    // // Wait for the tasks to finish.
    // //
    // // We drop our sender first because the recv() call otherwise
    // // sleeps forever.
    // drop(send);
    drop(rx);
    tx.closed().await;

    // When every sender has gone out of scope, the recv call
    // will return with an error. We ignore the error.
    // let _ = recv.recv().await;

    println!("Tasks complete.");

    Ok(())
}
