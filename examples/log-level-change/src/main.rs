use fregate::{
    axum::{routing::get, Router},
    bootstrap, get_handle_log_layer, http_trace_layer, Application, Empty,
};
use std::str::FromStr;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

// Change log level after 10 seconds.
// Default log level is INFO
// Will be changed to TRACE
#[tokio::main]
async fn main() {
    let conf = bootstrap::<Empty, _>([]);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let log_filter_reloader = get_handle_log_layer().unwrap();

        log_filter_reloader
            .modify(|filter| *filter.filter_mut() = EnvFilter::from_str("trace").unwrap())
            .unwrap()
    });

    let rest = Router::new()
        .route("/", get(handler))
        .layer(http_trace_layer());

    Application::new(&conf).router(rest).serve().await.unwrap();
}

async fn handler() -> &'static str {
    "Hello, World!"
}

/*
    curl http://0.0.0.0:8000
*/
