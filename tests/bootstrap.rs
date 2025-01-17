mod bootstrap_fn_test {
    use fregate::{bootstrap, get_handle_log_layer, Empty};
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    #[tokio::test]
    async fn bootstrap_test() {
        let config = bootstrap::<Empty, _>([]);

        // bootstrap() fn internally spawns tasks, let's give it a little time
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert_eq!(config.port, 8000);
        assert_eq!(config.host, IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        assert_eq!(config.private, Empty {});

        let logger = config.logger;
        assert_eq!(logger.traces_endpoint, None);
        assert_eq!(logger.service_name, "Rust".to_owned());
        assert_eq!(logger.trace_level, "info".to_owned());
        assert_eq!(logger.log_level, "info".to_owned());

        assert!(get_handle_log_layer().is_some());
    }
}
