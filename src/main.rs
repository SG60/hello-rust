use anyhow::Result;
use hello_rust_backend::do_some_stuff_with_etcd;
use std::time::Duration;
use tokio::signal;
use tokio::sync::watch;

// tracing
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_semantic_conventions as semcov;
use tracing::{event, span, Level};
use tracing_subscriber::{
    filter::Targets, fmt, layer::SubscriberExt, util::SubscriberInitExt, Layer,
};

use trace_output_fmt::JsonWithTraceId;

mod aws;
mod cluster_management;
mod notion_api;
mod settings;
mod trace_output_fmt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    set_up_logging()?;

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

    dbg!(std::env::var("NO_OTLP")
        .unwrap_or_else(|_| "0".to_owned())
        .as_str());

    if settings_map.etcd_url.is_some() {
        event!(Level::INFO, "About to try talking to etcd!");

        let result =
            do_some_stuff_with_etcd(&settings_map.etcd_url.expect("should be valid string")).await;

        match result {
            Ok(result) => {
                dbg!("{:#?}", &result);
                event!(Level::INFO, "{:#?}", result);
            }
            Err(error) => event!(Level::ERROR, "Error while talking to etcd. {}", error),
        }
        event!(Level::INFO, "Finished talking to etcd.");
    } else {
        event!(Level::WARN, "No etcd endpoint set.")
    }

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
        println!("Task shutting down. ({})", message);

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

    // Shutdown trace pipeline
    opentelemetry::global::shutdown_tracer_provider();

    println!("Tasks complete.");

    Ok(())
}

/// Set up an OTEL pipeline when the OTLP endpoint is set. Otherwise just set up tokio tracing
/// support.
fn set_up_logging() -> Result<()> {
    // Install a new OpenTelemetry trace pipeline
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        // with_env() gets OTEL endpoint from the env var OTEL_EXPORTER_OTLP_ENDPOINT
        // (if it is available)
        .with_exporter(opentelemetry_otlp::new_exporter().tonic().with_env())
        // config, service.name etc.
        .with_trace_config(opentelemetry::sdk::trace::config().with_resource(
            opentelemetry::sdk::Resource::new(vec![
                semcov::resource::SERVICE_NAME.string(env!("CARGO_PKG_NAME")),
                semcov::resource::SERVICE_VERSION.string(env!("CARGO_PKG_VERSION")),
            ]),
        ))
        .install_batch(opentelemetry::runtime::TokioCurrentThread)?;

    let global_tracing_filter = Targets::default().with_target("hello_rust_backend", Level::TRACE);

    // Create a tracing layer with the configured tracer
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let fmt_layer = fmt::Layer::default().json().event_format(JsonWithTraceId);

    // Include an option for when there is no otlp endpoint available. In this case, pretty print
    // events, as the data doesn't need to be nicely formatted json for analysis.
    match std::env::var("NO_OTLP")
        .unwrap_or_else(|_| "0".to_owned())
        .as_str()
    {
        "0" => {
            // The SubscriberExt and SubscriberInitExt traits are needed to extend the
            // Registry to accept `opentelemetry (the OpenTelemetryLayer type).
            let tracing_subscriber_registry = tracing_subscriber::registry()
                .with(opentelemetry)
                // Continue logging to stdout as well
                .with(fmt_layer)
                // Add a filter to the layer so that it only observes the spans that I want
                .with(global_tracing_filter);

            tracing_subscriber_registry.try_init()?
        }
        _ => {
            let tracing_subscriber_registry_no_otel = tracing_subscriber::registry()
                .with(fmt::Layer::default().pretty())
                .with(global_tracing_filter);

            tracing_subscriber_registry_no_otel.try_init()?
        }
    };

    Ok(())
}
