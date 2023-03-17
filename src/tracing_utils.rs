//! Utilities for tracing and logging.
use anyhow::Result;
// tracing
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_semantic_conventions as semcov;
use tonic::service::Interceptor;
use tracing::Level;
use tracing_subscriber::{filter::Targets, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use self::trace_output_fmt::JsonWithTraceId;

pub mod trace_output_fmt;

/// Set up an OTEL pipeline when the OTLP endpoint is set. Otherwise just set up tokio tracing
/// support.
pub fn set_up_logging() -> Result<()> {
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

#[derive(Clone)]
pub struct GrpcInterceptor;
impl Interceptor for GrpcInterceptor {
    fn call(&mut self, req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        Ok(req)
    }
}

pub type InterceptedGrpcService =
    tonic::codegen::InterceptedService<tonic::transport::Channel, GrpcInterceptor>;
