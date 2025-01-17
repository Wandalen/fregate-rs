use opentelemetry::{global, sdk, sdk::trace::Tracer, sdk::Resource, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_zipkin::B3Encoding::MultipleHeader;
use std::str::FromStr;
use time::format_description::well_known::Rfc3339;
use tokio::sync::OnceCell;
use tracing_opentelemetry::OpenTelemetryLayer as OTLayer;
use tracing_subscriber::{
    filter::EnvFilter,
    filter::Filtered,
    fmt,
    fmt::{
        format::{FmtSpan, Format, Json, JsonFields},
        layer,
        time::UtcTime,
    },
    layer::{Layered, SubscriberExt},
    registry, reload,
    reload::Handle,
    util::SubscriberInitExt,
    Layer, Registry,
};

static HANDLE_LOG_LAYER: OnceCell<HandleLogLayer> = OnceCell::const_new();

/// Return Some(&'static HandleLogLayer) if Handler is set up, otherwise return None
///
/// Used to change log level filter
pub fn get_handle_log_layer() -> Option<&'static HandleLogLayer> {
    HANDLE_LOG_LAYER.get()
}

type DefaultLayer = fmt::Layer<Registry, JsonFields, Format<Json, UtcTime<Rfc3339>>>;
type DefaultLayered = Layered<LogLayer, Registry>;
type LogFiltered = Filtered<DefaultLayer, EnvFilter, Registry>;
/// Shortcut for log Layer type
pub type LogLayer = reload::Layer<LogFiltered, Registry>;
/// Shortcut for log Layer handler type
pub type HandleLogLayer = Handle<LogFiltered, Registry>;
type TraceFiltered = Filtered<OTLayer<DefaultLayered, Tracer>, EnvFilter, DefaultLayered>;
// TODO: will be changed to reload
/// Shortcut for trace Layer type
pub type TraceLayer = TraceFiltered;
/// Shortcut for trace Layer handler type
pub type HandleTraceLayer = Handle<TraceFiltered, DefaultLayered>;

fn get_log_layers(log_level: &str) -> (LogLayer, HandleLogLayer) {
    let log_level = EnvFilter::from_str(log_level).unwrap_or_default();

    let log_filter = layer()
        .json()
        .with_timer(UtcTime::rfc_3339())
        .flatten_event(true)
        .with_target(true)
        .with_span_list(false)
        .with_current_span(false)
        // TODO(kos): This probably should be `FmtSpan::ACTIVE`?
        .with_span_events(FmtSpan::NONE)
        .with_filter(log_level);

    reload::Layer::new(log_filter)
}

#[allow(clippy::expect_used)]
fn get_trace_layer(trace_level: &str, service_name: &str, traces_endpoint: &str) -> TraceLayer {
    global::set_text_map_propagator(opentelemetry_zipkin::Propagator::with_encoding(
        MultipleHeader,
    ));

    let tracer =
        opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(traces_endpoint),
            )
            .with_trace_config(sdk::trace::config().with_resource(Resource::new(vec![
                KeyValue::new("service.name", service_name.to_owned()),
            ])))
            // FIXME(kos): ?
            .install_batch(opentelemetry::runtime::Tokio)
            .expect("failed to install opentelemetry_otlp pipeline");
    // FIXME(kos): `get_trace_layer` should return Result<>.
    //             Caller of the function should handle the result properly.
    //             What is proper strategy of handling this error?
    //             Probably starting service and logging error, not halting.

    let trace_level = EnvFilter::from_str(trace_level).unwrap_or_default();

    tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(trace_level)

    // TODO: bug ? trace_id is not generated when used with reload Layer
    // let (traces_filter, traces_filter_reloader) = reload::Layer::new(opentelemetry_layer);
    // settings
    //     .traces_filter_reloader
    //     .replace(traces_filter_reloader);
}

fn set_panic_hook() {
    // Capture the span context in which the program panicked
    std::panic::set_hook(Box::new(|panic| {
        // If the panic has a source location, record it as structured fields.
        if let Some(location) = panic.location() {
            tracing::error!(
                message = %panic,
                panic.file = location.file(),
                panic.line = location.line(),
                panic.column = location.column(),
            );
        } else {
            tracing::error!(message = %panic);
        }
    }));
}

/// Set up global subscriber with formatting log layer to print logs in json format to console and if traces_endpoint is provided opentelemetry exporter to send traces to grafana
///
/// Panics if:
///
/// Called out of tokio runtime
///
/// Called twice
///
/// Fails to set up opentelemetry_otlp pipeline
pub fn init_tracing(
    log_level: &str,
    trace_level: &str,
    service_name: &str,
    traces_endpoint: Option<&str>,
) {
    let (lag_layer, log_layer_handle) = get_log_layers(log_level);

    let trace_layer = traces_endpoint
        .map(|traces_endpoint| get_trace_layer(trace_level, service_name, traces_endpoint));

    // This will panic if called twice
    registry().with(lag_layer).with(trace_layer).init();

    // TODO(kos): No need for async `OnceCell` here, as the initialization code
    //            contains no `.await` points. Sync `OnceCell` better be used
    //            instead.
    tokio::task::spawn(async {
        let _handle = HANDLE_LOG_LAYER
            .get_or_init(|| async { log_layer_handle })
            .await;
    });

    set_panic_hook();
}
