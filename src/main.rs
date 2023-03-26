use anyhow::Result;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

// tracing
use hello_rust_backend::tracing_utils::set_up_logging;
use tracing::{event, span, Instrument, Level};

use hello_rust_backend::do_some_stuff_with_etcd;

#[tokio::main]
async fn main() -> Result<()> {
    set_up_logging()?;

    // let (send, mut recv): (Sender<()>, _) = channel(1);
    let (tx, rx) = watch::channel(());

    // Env vars! -----------------------------------
    let mut retry_wait_seconds = 1;
    let settings_map = loop {
        let settings_map = hello_rust_backend::settings::get_settings();
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
    event!(Level::INFO, "Settings successfully obtained.");
    event!(Level::INFO, "{:#?}", settings_map);

    dbg!(std::env::var("NO_OTLP")
        .unwrap_or_else(|_| "0".to_owned())
        .as_str());

    let span = span!(Level::TRACE, "talk to etcd");
    async {
        // This is correct! If we yield here, the span will be exited,
        // and re-entered when we resume.
        if settings_map.etcd_url.is_some() {
            event!(Level::INFO, "About to try talking to etcd!");

            event!(Level::INFO, "Clustered setting: {}", settings_map.clustered);

            let result =
                do_some_stuff_with_etcd(&settings_map.etcd_url.expect("should be valid string"))
                    .await;

            match result {
                Ok(result) => {
                    dbg!("{:#?}", &result);
                    event!(Level::INFO, "{:#?}", result);
                }
                Err(error) => event!(Level::ERROR, "Error while talking to etcd. {:#?}", error),
            }
            event!(Level::INFO, "Finished talking to etcd.");
        } else {
            event!(Level::WARN, "No etcd endpoint set.")
        }
    }
    // instrument the async block with the span...
    .instrument(span)
    // ...and await it.
    .await;

    async fn some_operation(message: &str, duration: Duration, receiver: watch::Receiver<()>) {
        loop {
            tokio::time::sleep(duration).await;

            let span = span!(Level::TRACE, "message span");
            let _enter = span.enter();
            event!(Level::INFO, message);

            if receiver.has_changed().unwrap_or(true) {
                break;
            };
        }
        event!(Level::INFO, "Task shutting down. ({})", message);

        // sender goes out of scope ...
    }

    let _op1 = tokio::spawn(some_operation(
        "Hello World!",
        Duration::from_secs(10),
        rx.clone(),
    ));

    let _op2 = tokio::spawn(some_operation(
        "hello world from a shorter loop!",
        Duration::from_secs(7),
        rx.clone(),
    ));

        }
    let mut rx2 = rx.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = loop_getting_cluster_members() => {},
            _ = rx2.changed() => {
                dbg!("rx shutdown channel changed");
             }
        }
    });

    let mut sigterm_stream = signal(SignalKind::terminate())?;
    let mut sigint_stream = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = sigterm_stream.recv() => {}
        _ = sigint_stream.recv() => {}
    }

    let span = span!(Level::TRACE, "Shutting down tasks");
    async {
        // send shutdown signal to application and wait
        tx.send(())?;

        // // Wait for the tasks to finish.
        // //
        // // We drop our sender first because the recv() call otherwise
        // // sleeps forever.
        // drop(send);
        drop(rx);
        tx.closed().await;

        event!(Level::TRACE, "All tasks shutdown.");

        // When every sender has gone out of scope, the recv call
        // will return with an error. We ignore the error.
        // let _ = recv.recv().await;

        Ok::<(), anyhow::Error>(())
    }
    // instrument the async block with the span...
    .instrument(span)
    // ...and await it.
    .await?;

    // Shutdown trace pipeline
    opentelemetry::global::shutdown_tracer_provider();

    println!("Shutdown complete!");

    Ok(())
}
