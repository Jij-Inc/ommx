fn main() -> std::process::ExitCode {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
    std::process::ExitCode::from(ommx::cli::run(std::env::args_os()))
}
