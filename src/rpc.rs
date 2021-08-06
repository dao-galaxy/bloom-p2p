use std::collections::HashMap;
use std::sync::{atomic, Arc, RwLock};
use std::thread;

use crate::server::{BloomHandler, PeerInfo, TxPool};
use devp2p::{NetworkService, NodeId};
use jsonrpc_core::futures::Future;
use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use jsonrpc_pubsub::typed;
use jsonrpc_pubsub::{PubSubHandler, Session, SubscriptionId};
use network::PeerId;
use network_devp2p as devp2p;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize)]
pub struct Info {
    external_url: Option<String>,
    local_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Peers {
    sync_peers: HashMap<PeerId, PeerInfo>,
    service_peers: Vec<PeerId>,
}

#[derive(Serialize, Deserialize)]
pub struct Node {
    pub buck_id: u8,
    pub node_id: NodeId,
    pub address: SocketAddr,
}

#[rpc]
pub trait Rpc {
    type Metadata;

    /// Hello subscription
    #[pubsub(subscription = "peers", subscribe, name = "peers_sub")]
    fn subscribe(&self, meta: Self::Metadata, subscriber: typed::Subscriber<Peers>, param: u64);

    /// Unsubscribe from hello subscription.
    #[pubsub(subscription = "peers", unsubscribe, name = "peers_unsub")]
    fn unsubscribe(
        &self,
        meta: Option<Self::Metadata>,
        subscription: SubscriptionId,
    ) -> Result<bool>;

    /// get p2p info
    #[rpc(name = "info")]
    fn info(&self) -> Result<Info>;

    /// get peers
    #[rpc(name = "peers")]
    fn peers(&self) -> Result<Peers>;

    /// send tx
    #[rpc(name = "send_tx")]
    fn send_tx(&self, tx_rlp: String, is_hex: bool) -> Result<bool>;

    /// get tx pool
    #[rpc(name = "tx_pool")]
    fn tx_pool(&self) -> Result<TxPool>;

    /// clear tx pool
    #[rpc(name = "clear_tx_pool")]
    fn clear_tx_pool(&self) -> Result<()>;

    /// get node buckets
    #[rpc(name = "node_buckets")]
    fn node_buckets(&self) -> Result<Vec<Node>>;
}

struct RpcImpl {
    uid: atomic::AtomicUsize,
    active: Arc<RwLock<HashMap<SubscriptionId, typed::Sink<Peers>>>>,
    network: Arc<NetworkService>,
    handler: Arc<BloomHandler>,
}

impl Rpc for RpcImpl {
    type Metadata = Arc<Session>;

    fn subscribe(&self, _meta: Self::Metadata, subscriber: typed::Subscriber<Peers>, param: u64) {
        if param != 10 {
            subscriber
                .reject(Error {
                    code: ErrorCode::InvalidParams,
                    message: "Rejecting subscription - invalid parameters provided.".into(),
                    data: None,
                })
                .unwrap();
            return;
        }

        let id = self.uid.fetch_add(1, atomic::Ordering::SeqCst);
        let sub_id = SubscriptionId::Number(id as u64);
        let sink = subscriber.assign_id(sub_id.clone()).unwrap();
        self.active.write().unwrap().insert(sub_id, sink);
    }

    fn unsubscribe(&self, _meta: Option<Self::Metadata>, id: SubscriptionId) -> Result<bool> {
        let removed = self.active.write().unwrap().remove(&id);
        if removed.is_some() {
            Ok(true)
        } else {
            Err(Error {
                code: ErrorCode::InvalidParams,
                message: "Invalid subscription.".into(),
                data: None,
            })
        }
    }

    fn info(&self) -> Result<Info> {
        Ok(Info {
            external_url: self.network.external_url(),
            local_url: self.network.local_url(),
        })
    }

    fn peers(&self) -> Result<Peers> {
        Ok(Peers {
            sync_peers: self.handler.get_peers().clone(),
            service_peers: self.network.connected_peers(),
        })
    }

    fn send_tx(&self, tx_rlp: String, is_hex: bool) -> Result<bool> {
        let res = self
            .handler
            .receive_tx_from_local(&tx_rlp, is_hex, self.network.clone());
        Ok(res)
    }

    fn tx_pool(&self) -> Result<TxPool> {
        Ok(self.handler.get_tx_pool())
    }

    fn clear_tx_pool(&self) -> Result<()> {
        Ok(self.handler.clear_tx_pool())
    }

    fn node_buckets(&self) -> Result<Vec<Node>> {
        let mut res = vec![];
        if let Some(buckets) = self.network.get_node_buckets() {
            for i in 0..256 {
                for node in &buckets[i] {
                    res.push(Node {
                        buck_id: i as u8,
                        node_id: node.id,
                        address: node.endpoint.address,
                    });
                }
            }
        }
        Ok(res)
    }
}

pub fn start_rpc_server(
    addr: &SocketAddr,
    network: Arc<NetworkService>,
    handler: Arc<BloomHandler>,
) {
    let mut io = PubSubHandler::default();
    let rpc = RpcImpl {
        uid: atomic::AtomicUsize::default(),
        active: Arc::new(RwLock::new(HashMap::new())),
        handler: handler.clone(),
        network: network.clone(),
    };

    let active_subscriptions = rpc.active.clone();

    let h = handler.clone();
    let n = network.clone();
    thread::spawn(move || loop {
        {
            let subscribers = active_subscriptions.read().unwrap();
            for sink in subscribers.values() {
                let peers = Ok(Peers {
                    sync_peers: h.get_peers().clone(),
                    service_peers: n.connected_peers(),
                });
                let _ = sink.notify(peers).wait();
            }
        }
        thread::sleep(::std::time::Duration::from_secs(3));
    });

    io.extend_with(rpc.to_delegate());

    let server = jsonrpc_tcp_server::ServerBuilder::with_meta_extractor(
        io,
        |context: &jsonrpc_tcp_server::RequestContext| {
            Arc::new(Session::new(context.sender.clone()))
        },
    )
    .start(addr)
    .expect("Server must start with no issues");

    server.wait()
}
