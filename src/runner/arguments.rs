
use clap::{Parser, ValueEnum};
#[derive(Parser, Debug)]
// #[command(author, version, about, long_about = None)]
#[command()]
pub struct Args {
   /// app execution mode.
   #[arg(short, long, value_enum)]
   pub mode: Mode,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum)]
pub enum Mode {
   /// Server mode.
   Server,
   /// Client mode.
   Client
}
