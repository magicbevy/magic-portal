mod cli;
mod core;
mod platform;
mod theme;

fn main() {
    std::process::exit(cli::run());
}
