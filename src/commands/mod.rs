mod client_cmd;
mod server_cmd;

use client_cmd::ClientCmd;
use server_cmd::ServerCmd;
use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
pub enum Subcommand {
    Server(ServerCmd),
    Client(ClientCmd),
}

impl Subcommand {
    pub fn run(&self) {
        match self {
            Subcommand::Server(cmd) => {
                cmd.run();
            }
            Subcommand::Client(cmd) => {
                cmd.run();
            }
        }
    }
}
