use structopt::StructOpt;

use crate::server;

#[derive(Debug, Clone, StructOpt)]
pub struct ServerCmd {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, Clone, StructOpt)]
enum Command {
    /// Query external or contract account information
    Run {
        /// Local listen port
        #[structopt(long = "port", default_value = "33030")]
        port: u16,
        /// Enr key and nodes location
        #[structopt(long = "data-dir", default_value = "data")]
        data_dir: String,
        /// Boot Node
        #[structopt(long = "boot-node")]
        boot_node: Option<String>,
        /// Log level
        #[structopt(long = "log-level", default_value = "info")]
        log_level: String,
    },
}

impl ServerCmd {
    pub fn run(&self) {
        match &self.cmd {
            Command::Run {
                port,
                data_dir,
                boot_node,
                log_level,
            } => {
                server::run(*port, data_dir, boot_node, log_level);
            }
        }
    }
}
