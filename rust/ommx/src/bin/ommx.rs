fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(ommx::cli::run(std::env::args_os()))
}
