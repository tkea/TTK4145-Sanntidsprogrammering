use rand;
use rand::Rng;

use std::rc::Rc;
use std::thread;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};

use elevator_driver::elev_io::{N_FLOORS, Floor, Button, MotorDir, Light};

use network::localip::get_localip;
use network::peer::{PeerTransmitter, PeerReceiver, PeerUpdate};
use network::bcast::{BcastTransmitter, BcastReceiver};

use request_handler::request::*;
use request_handler::request::RequestStatus::*;

const PEER_PORT: u16 = 9877;
const BCAST_PORT: u16 = 9876;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BroadcastMessage {
    RequestMessage(Request),
    Position(usize),
}

fn spawn_peer_update_threads(peer_tx: Sender<PeerUpdate<String>>) {
    let unique = rand::thread_rng().gen::<u16>();

    thread::spawn(move|| {
        let id = format!("{}:{}", get_localip().unwrap(), unique);
        PeerTransmitter::new(PEER_PORT)
            .expect("Error creating PeerTransmitter")
            .run(&id);
    });

    thread::spawn(move|| {
        PeerReceiver::new(PEER_PORT)
            .expect("Error creating PeerReceiver")
            .run(peer_tx);
    });
}

fn spawn_bcast_threads(transmit_rx: Receiver<BroadcastMessage>, receive_tx: Sender<(BroadcastMessage, IP)>) {
    thread::spawn(move|| {
        BcastTransmitter::new(BCAST_PORT)
            .expect("Error creating ")
            .run(transmit_rx);
    });

    thread::spawn(move|| {
        BcastReceiver::new(BCAST_PORT)
            .expect("Error creating BcastReceiver")
            .run(receive_tx);
    });
}

pub struct RequestTransmitter {
    pub bcast_sender: Sender<BroadcastMessage>,
    pub bcast_receiver: Receiver<(BroadcastMessage, IP)>,
    pub peer_receiver: Receiver<PeerUpdate<IP>>,
}

impl RequestTransmitter {
    pub fn new() -> Self {
        let (peer_tx, peer_rx) = channel::<PeerUpdate<IP>>();
        spawn_peer_update_threads(peer_tx);

        let (bcast_transmitter_tx, bcast_transmitter_rx) = channel::<BroadcastMessage>();
        let (bcast_receiver_tx, bcast_receiver_rx) = channel::<(BroadcastMessage, IP)>();
        spawn_bcast_threads(bcast_transmitter_rx, bcast_receiver_tx);

        RequestTransmitter {
            bcast_sender: bcast_transmitter_tx,
            bcast_receiver: bcast_receiver_rx,
            peer_receiver: peer_rx,
        }
    }

    pub fn announce_request(&self, request: Request) {
        self.bcast_sender.send(BroadcastMessage::RequestMessage(request))
            .expect("Could not announce request");
    }
}
