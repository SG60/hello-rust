//! Utilities for tracing and logging.

use anyhow::Result;
use opentelemetry_http::HeaderInjector;
// tracing
use opentelemetry::{global, sdk::propagation::TraceContextPropagator};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_semantic_conventions as semcov;
use tonic::service::Interceptor;
use tracing::{Level, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{
    filter::{LevelFilter, Targets},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

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
        .with(
            layers.with_filter(
                Targets::new()
                    .with_target("hello_rust_backend", Level::TRACE)
                    .with_default(LevelFilter::INFO),
            ),
        );

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

        let mut headers = req.metadata().clone().into_headers();
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&context, &mut HeaderInjector(&mut headers));
        });

        // metadata map (which is an encapsulated HeaderMap)
        let mut_meta = req.metadata_mut();

        for i in ["traceparent", "tracestate"] {
            if let Some(header) = headers.get(i) {
                mut_meta.insert(
                    i,
                    header
                        .to_str()
                        .expect("should be valid string")
                        .parse()
                        .expect("should be valid string and parse"),
                );
            };
        }

        Ok(req)
    }
}

pub type InterceptedGrpcService =
    tonic::codegen::InterceptedService<tonic::transport::Channel, GrpcInterceptor>;
