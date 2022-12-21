use clap::Parser;
use runner::arguments::{Args, Mode};
mod client;
mod server;
mod runner;

fn main() {
    let args = Args::parse();
    match args.mode {
        Mode::Server => {
            server::run();
        }
        Mode::Client => {
            client::run();
        }
    }
}
