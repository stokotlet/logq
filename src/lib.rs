mod cli;
mod index;
mod parse;
mod scan;

pub fn run() -> std::process::ExitCode {
    cli::run()
}
