use anyhow::Result;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{event, span, Instrument, Level};

#[tokio::main]
async fn main() -> Result<()> {
    let (tx, rx) = tokio::sync::watch::channel(());

    let app_run_join_handle = tokio::spawn(hello_rust_backend::run(rx.clone()));

    let mut sigterm_stream = signal(SignalKind::terminate())?;
    let mut sigint_stream = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = sigterm_stream.recv() => {event!(Level::INFO, "sigterm received");}
        _ = sigint_stream.recv() => {event!(Level::INFO, "sigint received");}
        // also quit if the work task has completed
        result = app_run_join_handle => {
            match result {
                Ok(_) => {event!(Level::INFO, "work finished");},
                Err(error) => {
                    event!(Level::ERROR, ?error, "Work task panicked");
                }
            }
        }
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
