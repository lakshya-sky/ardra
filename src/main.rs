mod game;
use std::{error::Error, time::Duration};

//use anyhow::{Ok, Result};
use libp2p::{
    core::upgrade,
    floodsub::{self, Floodsub, FloodsubEvent},
    futures::StreamExt,
    identity, mdns, mplex, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, Multiaddr, PeerId, Swarm, Transport,
};

use tokio::sync::{mpsc, oneshot};

// Write a function that returns a PeerId and a Keypair.
fn random_peer_id() -> (PeerId, identity::Keypair) {
    // Generate a random Ed25519 keypair.
    let id_keys = identity::Keypair::generate_ed25519();
    // Get the PeerId from the public key.
    let peer_id = PeerId::from(id_keys.public());
    (peer_id, id_keys)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (local_peer_id, local_keypair) = random_peer_id();
    //let (response_sender, mut response_rcv) = mpsc::unbounded_channel();
    let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(
            noise::NoiseAuthenticated::xx(&local_keypair)
                .expect("Signing libp2p-noise static DH keypair failed."),
        )
        .multiplex(mplex::MplexConfig::new())
        .boxed();
    // Create a floodsub topic
    let floodsub_topic = floodsub::Topic::new("chat");

    // We create a custom  behaviour that combines floodsub and mDNS.
    // The derive generates a delegating `NetworkBehaviour` impl.
    #[derive(NetworkBehaviour)]
    #[behaviour(out_event = "MyBehaviourEvent")]
    struct MyBehaviour {
        floodsub: Floodsub,
        mdns: mdns::tokio::Behaviour,
    }

    enum MyBehaviourEvent {
        Floodsub(FloodsubEvent),
        Mdns(mdns::Event),
    }

    impl From<FloodsubEvent> for MyBehaviourEvent {
        fn from(event: FloodsubEvent) -> Self {
            MyBehaviourEvent::Floodsub(event)
        }
    }

    impl From<mdns::Event> for MyBehaviourEvent {
        fn from(event: mdns::Event) -> Self {
            MyBehaviourEvent::Mdns(event)
        }
    }

    // Create a Swarm to manage peers and events.
    let mdns_behaviour = mdns::Behaviour::new(Default::default(), local_peer_id)?;
    let mut behaviour = MyBehaviour {
        floodsub: Floodsub::new(local_peer_id),
        mdns: mdns_behaviour,
    };

    behaviour.floodsub.subscribe(floodsub_topic.clone());

    let mut swarm = Swarm::with_tokio_executor(transport, behaviour, local_peer_id);

    //React out to another node if specified
    if let Some(node_name) = std::env::args().nth(1) {
        let alpha_addr = "/ip4/0.0.0.0/tcp/56000".parse()?;
        if node_name == "alpha" {
            swarm.listen_on(alpha_addr)?;
        } else {
            let addr: Multiaddr = "/ip4/0.0.0.0/tcp/56001".parse()?;
            swarm.listen_on(addr.clone())?;
            swarm.dial(alpha_addr.clone())?;
            println!("Dialed {alpha_addr:?}");
        }
    }

    let (inbound_tx, mut inbound_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
    let local_game = game::Game::new(inbound_rx, outbound_tx, cancel_rx);

    tokio::spawn(async move { game::run(local_game).await });

    loop {
        tokio::select! {
            msg = outbound_rx.recv() => {
                if let Some(msg) = msg {
                    println!("Sending: {:?}", String::from_utf8_lossy(&msg));
                    swarm.behaviour_mut().floodsub.publish(floodsub_topic.clone(), msg);
                }
            },
            signal = tokio::signal::ctrl_c() => {
                println!("Ctrl-C received, shutting down...");
                if signal.is_ok() {
                    cancel_tx.send(()).unwrap();
                    break;
                }
            },
            event = swarm.select_next_some() =>{
                match event{
                    SwarmEvent::NewListenAddr{ address, .. } => {
                        println!("Listening on {:?}", address);
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(event)) => {
                        match event {
                            mdns::Event::Discovered(list) => {
                                for (peer, _) in list {
                                    swarm.behaviour_mut().floodsub.add_node_to_partial_view(peer);
                                }
                            }
                             mdns::Event::Expired(list) => {
                                 for (peer, _) in list {
                                     if !swarm.behaviour().mdns.has_node(&peer) {
                                         swarm.behaviour_mut().floodsub.remove_node_from_partial_view(&peer);
                                     }
                                 }
                             }
                        }
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Floodsub(FloodsubEvent::Message(message))) => {
                        println!("Received: '{:?}' from {:?}", String::from_utf8_lossy(&message.data), message.source);
                        inbound_tx.send(message.data).unwrap();
                    }
                    _ => {}
                }
            },
            //msg = inbound_rx.recv() => {
            //    if let Some(msg) = msg {
            //        println!("Received: {:?}", String::from_utf8_lossy(&msg));
            //    }
            //}
        }
    }
    println!("Exiting");

    Ok(())
}
