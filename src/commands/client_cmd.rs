use serde_json;
use std::io::prelude::*;
use std::net::TcpStream;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
pub struct ClientCmd {
    #[structopt(long = "server", default_value = "127.0.0.1")]
    server: String,
    #[structopt(long = "port", default_value = "23030")]
    port: u16,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, Clone, StructOpt)]
enum Command {
    /// show p2p info
    Info {},
    /// list connected peers
    Peers {},
    /// show tx pool
    Tx_pool {},
    /// show node buckets
    Node_buckets {},
    /// send tx to p2p network
    Send_tx {
        /// tx data
        #[structopt()]
        body: String,

        /// tx type hex
        #[structopt(long = "hex")]
        hex: bool,
    },
    /// subscribe to show peers
    Peers_sub {
        /// subscribe param
        #[structopt(default_value = "10")]
        level: u64,
    },
    /// unsubscribe to show peers
    Peers_unsub {
        /// subscribe id
        #[structopt()]
        sub_id: u64,
    },
}

impl ClientCmd {
    fn rpc_request(&self, stream: &mut TcpStream, method: &str, params_json: &str) -> String {
        let request = format!(
            "{{ \"jsonrpc\":\"2.0\", \"id\": 1, \"method\": \"{}\", \"params\": {} }}\n",
            method, params_json
        );
        stream
            .write_all(request.as_bytes())
            .expect("write_all failed!!!");
        let res = &mut [0; 2048];
        stream.read(res).expect("read failed!!!");
        String::from_utf8(res.to_vec()).unwrap()
    }

    fn rpc_subscribe(&self, stream: &mut TcpStream, method: &str, params_json: &str) {
        let request = format!(
            "{{ \"jsonrpc\":\"2.0\", \"id\": 1, \"method\": \"{}\", \"params\": {} }}\n",
            method, params_json
        );
        stream
            .write_all(request.as_bytes())
            .expect("write_all failed!!!");
        let res = &mut [0; 2048];
        loop {
            stream.read(res).expect("read failed!!!");
            let res = String::from_utf8(res.to_vec()).unwrap();
            println!("{}", res);
        }
    }

    pub fn run(&self) {
        let addr = SocketAddr::V4(SocketAddrV4::new(self.server.parse().unwrap(), self.port));
        let mut stream = TcpStream::connect(addr).expect("Cann't connect rpc server!!!");
        match &self.cmd {
            Command::Info {} => {
                let method = "info";
                let res = self.rpc_request(&mut stream, method, "[]");
                println!("response for {}", method);
                println!("-----------------------------");
                println!("{}", res);
            }
            Command::Peers {} => {
                let method = "peers";
                let res = self.rpc_request(&mut stream, method, "[]");
                println!("response for {}", method);
                println!("-----------------------------");
                println!("{}", res);
            }
            Command::Node_buckets {} => {
                let method = "node_buckets";
                let res = self.rpc_request(&mut stream, method, "[]");
                println!("response for {}", method);
                println!("-----------------------------");
                println!("{}", res);
            }
            Command::Tx_pool {} => {
                let method = "tx_pool";
                let res = self.rpc_request(&mut stream, method, "[]");
                println!("response for {}", method);
                println!("-----------------------------");
                println!("{}", res);
            }
            Command::Send_tx { body, hex } => {
                let method = "send_tx";
                let res =
                    self.rpc_request(&mut stream, method, &format!("[\"{}\", {}]", body, hex));
                println!("response for {}", method);
                println!("-----------------------------");
                println!("{}", res);
            }
            Command::Peers_sub { level } => {
                let method = "peers_sub";
                println!("response for {}", method);
                println!("-----------------------------");
                self.rpc_subscribe(&mut stream, method, &format!("[{}]", level));
            }
            Command::Peers_unsub { sub_id } => {
                let method = "peers_unsub";
                let res = self.rpc_request(&mut stream, method, &format!("[{}]", sub_id));
                println!("response for {}", method);
                println!("-----------------------------");
                println!("{}", res);
            }
        }
    }
}
