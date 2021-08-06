mod commands;
mod rpc;
mod server;

use commands::Subcommand;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Cli {
    #[structopt(subcommand)]
    pub subcmd: Option<Subcommand>,
}

fn main() {
    let cli = Cli::from_args();
    // println!("{:#?}", cli);

    if let Some(ref subcmd) = cli.subcmd {
        subcmd.run();
    } else {
        println!("Unknown command!!!");
    }
}
