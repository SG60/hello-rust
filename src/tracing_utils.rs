//! Utilities for tracing and logging.

use anyhow::Result;
use std::str::FromStr;

// tracing
use opentelemetry::{global, sdk::propagation::TraceContextPropagator};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_semantic_conventions as semcov;
use tonic::{metadata::MetadataKey, service::Interceptor};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use self::trace_output_fmt::JsonWithTraceId;

pub mod trace_output_fmt;

/// Set up an OTEL pipeline when the OTLP endpoint is set. Otherwise just set up tokio tracing
/// support.
pub fn set_up_logging() -> Result<()> {
    global::set_text_map_propagator(TraceContextPropagator::new());

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

    // Create a tracing layer with the configured tracer
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let fmt_layer = fmt::Layer::default().json().event_format(JsonWithTraceId);

    let otlp_enabled = std::env::var("NO_OTLP")
        .unwrap_or_else(|_| "0".to_owned())
        .as_str()
        == "0";

    let layers = match otlp_enabled {
        // Include an option for when there is no otlp endpoint available. In this case, pretty print
        // events, as the data doesn't need to be nicely formatted json for analysis.
        true => opentelemetry.and_then(fmt_layer).boxed(),
        false => fmt::Layer::default().pretty().boxed(),
    };

    let tracing_registry = tracing_subscriber::registry()
        // Add a filter to the layers so that they only observe the spans that I want
        .with(layers.with_filter(
            // Parse env filter from RUST_LOG, setting a default directive if that fails.
            // Syntax for directives is here: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::try_new("hello_rust_backend,warn")
                    .expect("hard-coded default directive should be valid")
            }),
        ));

    #[cfg(feature = "tokio-console")]
    let tracing_registry = tracing_registry.with(console_subscriber::spawn());

    tracing_registry.try_init()?;

    Ok(())
}

/// This interceptor adds tokio tracing opentelemetry headers to grpc requests.
/// Allows stitching together distributed traces!
#[derive(Clone)]
pub struct GrpcInterceptor;
impl Interceptor for GrpcInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        // get otel context from current tokio tracing span
        let context = Span::current().context();

        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&context, &mut MetadataInjector(req.metadata_mut()));
        });

        Ok(req)
    }
}

pub struct MetadataInjector<'a>(&'a mut tonic::metadata::MetadataMap);
impl<'a> opentelemetry::propagation::Injector for MetadataInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(key) = MetadataKey::from_str(key) {
            if let Ok(val) = value.parse() {
                self.0.insert(key, val);
            }
        }
    }
}

/// A tonic channel intercepted to provide distributed tracing context propagation.
pub type InterceptedGrpcService =
    tonic::codegen::InterceptedService<tonic::transport::Channel, GrpcInterceptor>;
