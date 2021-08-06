use network as net;
use network_devp2p as devp2p;

use crate::rpc;
use ctrlc;
use devp2p::NetworkService;
use env_logger;
use ethereum_types::H256;
use log::{debug, info, warn};
use net::*;
use parking_lot::{RwLock, RwLockReadGuard};
use rlp::{Rlp, RlpStream};
use rustc_hex::{FromHex, ToHex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tiny_keccak::{Hasher, Keccak};

pub const PROTOCAL: [u8; 3] = *b"blo";
pub const PACKET_HELLO: u8 = 0x01;
pub const PACKET_TX: u8 = 0x02;

/// Do a keccak 256-bit hash and return result.
pub fn keccak_256(data: &[u8]) -> [u8; 32] {
    let mut keccak = Keccak::v256();
    keccak.update(data);
    let mut output = [0u8; 32];
    keccak.finalize(&mut output);
    output
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub address: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tx {
    pub body: Vec<u8>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct TxPool {
    txs: Vec<Tx>,
    ids: HashSet<H256>,
}

#[derive(Debug)]
pub struct BloomHandler {
    peers: RwLock<HashMap<PeerId, PeerInfo>>,
    tx_pool: RwLock<TxPool>,
}

impl BloomHandler {
    pub fn get_peers(&self) -> RwLockReadGuard<HashMap<PeerId, PeerInfo>> {
        self.peers.read()
    }

    fn send_hello(&self, io: &dyn NetworkContext, peer: &PeerId) {
        info!("send hello to #{}", peer);
        let mut hello = RlpStream::new_list(2);
        hello.append(&format!("hello, {}", peer));
        hello.append(&"111111111111111111111111111111111".to_string());

        if let Err(e) = io.respond(PACKET_HELLO, hello.out()) {
            info!("Error sending status to #{}: {:?}", peer, e);
            io.disconnect_peer(*peer);
        }
    }

    fn handle_hello(&self, io: &dyn NetworkContext, peer: &PeerId, data: &[u8]) {
        let rlp_obj = Rlp::new(data);
        let hello_info: String = rlp_obj.at(0).unwrap().as_val().unwrap();
        let genesis_hash: String = rlp_obj.at(1).unwrap().as_val().unwrap();
        info!("receive hello from #{}: {}", peer, genesis_hash);
        let session_info = io.session_info(*peer).unwrap();

        let mut peers = self.peers.write();
        peers.insert(
            *peer,
            PeerInfo {
                address: session_info.remote_address,
            },
        );
    }

    fn handle_new_tx(&self, io: &dyn NetworkContext, peer: &PeerId, data: &[u8]) {
        let tid = H256::from(keccak_256(data));
        let body = if let Ok(body) = String::from_utf8(data.into()) {
            body
        } else {
            data.to_hex()
        };
        info!("receive tx from #{}: {}###{}", peer, tid.to_string(), body);

        let mut tp = self.tx_pool.write();

        if tp.ids.contains(&tid) {
            return;
        }

        tp.ids.insert(tid);
        tp.txs.push(Tx { body: data.into() });

        // broadcast tx
        for (p, _) in &(*self.peers.read()) {
            if p == peer {
                continue;
            }
            info!("broadcast tx {} to #{}", tid, peer);
            if let Err(e) = io.send(*p, PACKET_TX, data.into()) {
                info!("Error broadcast tx to #{}, {:?}", p, e);
                io.disconnect_peer(*p);
            }
        }
    }

    pub fn clear_tx_pool(&self) {
        let mut tp = self.tx_pool.write();
        tp.ids.clear();
        tp.txs.clear();
    }

    pub fn get_tx_pool(&self) -> TxPool {
        let res = (*self.tx_pool.read()).clone();
        self.clear_tx_pool();
        return res;
    }

    pub fn receive_tx_from_local(
        &self,
        body: &str,
        is_hex: bool,
        network: Arc<NetworkService>,
    ) -> bool {
        let raw_body: Vec<u8> = if is_hex {
            if let Ok(raw_body) = body.from_hex() {
                raw_body
            } else {
                return false;
            }
        } else {
            body.as_bytes().to_vec()
        };
        let tid = H256::from(keccak_256(&raw_body));
        info!("receive tx from rpc: {}###{}", tid, body);

        let mut tp = self.tx_pool.write();

        if tp.ids.contains(&tid) {
            return false;
        }

        tp.ids.insert(tid);
        tp.txs.push(Tx {
            body: raw_body.clone(),
        });

        network.with_context(PROTOCAL, |io| {
            for (p, _) in &(*self.get_peers()) {
                info!("broadcast tx {} to #{}", tid, p);
                if let Err(e) = io.send(*p, PACKET_TX, raw_body.clone()) {
                    info!("Error broadcast tx to #{}, {:?}", p, e);
                    io.disconnect_peer(*p);
                }
            }
        });

        return true;
    }
}

impl NetworkProtocolHandler for BloomHandler {
    fn initialize(&self, io: &dyn NetworkContext) {
        io.register_timer(0, Duration::from_secs(1)).unwrap();
    }

    fn read(&self, io: &dyn NetworkContext, peer: &PeerId, packet_id: u8, data: &[u8]) {
        info!(
            "=============Received packet_id: *{} ({} bytes) from peer: #{}",
            packet_id,
            data.len(),
            peer
        );

        match packet_id {
            PACKET_HELLO => self.handle_hello(io, peer, data),
            PACKET_TX => self.handle_new_tx(io, peer, data),
            _ => (),
        }
    }

    fn connected(&self, io: &dyn NetworkContext, peer: &PeerId) {
        let session_info = io.session_info(*peer).unwrap();
        info!(
            "===============Connected, peer: #{} ({})",
            peer, session_info.remote_address
        );

        self.send_hello(io, peer);
    }

    fn disconnected(&self, io: &dyn NetworkContext, peer: &PeerId) {
        let session_info = io.session_info(*peer).unwrap();
        info!(
            "===============Disconnected, peer: #{} ({})",
            peer, session_info.remote_address
        );
        let mut peers = self.peers.write();
        peers.remove(peer);
    }
}

pub fn run(port: u16, data_dir: &str, boot_node: &Option<String>, log_level: &str) {
    // setup logger
    env::set_var("RUST_LOG", log_level);
    env_logger::init();

    // init network configuration
    let mut net_config = NetworkConfiguration::new_with_port(port);

    if let Some(b) = boot_node {
        net_config.boot_nodes.push(b.to_owned());
    } else {
        warn!("Does not enter boot_node");
    };

    let reserve_node = "enode://a1337fd03a00d8b656c639819d7d88cd5aaf7eaf5c46b78de5538b4449ae1c43ad22f029d7a56063999ae4cfe01a722f884e1af54a6195a0e41c350874fef089@203.195.218.114:33030";
    net_config.boot_nodes.push(reserve_node.to_owned());

    let mut config_path = PathBuf::new();
    config_path.push(data_dir);
    config_path.push("enr");

    let mut net_config_path = PathBuf::new();
    net_config_path.push(data_dir);
    net_config_path.push("nodes");

    net_config.config_path = Some(config_path.to_str().unwrap().to_owned());
    net_config.net_config_path = Some(net_config_path.to_str().unwrap().to_owned());
    net_config.client_version = "Bloom-network".into();
    debug!("net_config: {:?}", net_config);

    // init and start network service
    let service =
        Arc::new(NetworkService::new(net_config, None).expect("Error creating network service"));
    service.start().expect("Error starting service");

    let handler = Arc::new(BloomHandler {
        peers: RwLock::new(HashMap::new()),
        tx_pool: RwLock::new(TxPool::default()),
    });
    // register bloom sync protocol
    service
        .register_protocol(handler.clone(), PROTOCAL, &[(1, 0x11)])
        .expect("register_protocol failed");

    // start rpc server
    let rpc_service = service.clone();
    let rpc_handler = handler.clone();
    thread::spawn(move || {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port - 10000));
        rpc::start_rpc_server(&addr, rpc_service, rpc_handler);
    });

    // keep running
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        service.stop();
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("Got it! Exiting...");
}
