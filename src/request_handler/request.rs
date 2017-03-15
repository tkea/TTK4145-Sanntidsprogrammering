#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(type_ascription)]

use rand;

use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};

use rand::Rng;

use network::localip::get_localip;
use network::peer::{PeerTransmitter, PeerReceiver, PeerUpdate};
use network::bcast::{BcastTransmitter, BcastReceiver};

use std::collections::HashMap;

use self::RequestStatus::*;

const PEER_PORT: u16 = 9877;
const BCAST_PORT: u16 = 9876;

const N_FLOORS: usize = 4;

pub type IP = String;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum RequestType {
    Internal = 2,
    CallUp = 1,
    CallDown = 0,
}

impl Default for RequestType {
    fn default() -> RequestType { RequestType::CallUp }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum RequestStatus {
    Active,
    Pending,
    Inactive,
    Unknown
}

impl Default for RequestStatus {
    fn default() -> RequestStatus { RequestStatus::Unknown }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Request {
    pub floor: usize,
    pub request_type: RequestType,
    pub status: RequestStatus,
    pub acknowledged_by: Vec<IP>,
}

impl Request {
    pub fn move_to_active(&mut self) -> RequestStatus {
        self.status = Active;
        Active
    }

    pub fn move_to_pending(&mut self) -> RequestStatus {
        self.status = Pending;
        Pending
    }

    pub fn move_to_inactive(&mut self) -> RequestStatus {
        self.status = Inactive;
        Inactive
    }

    pub fn handle_unknown_local(&mut self, remote: &Request) -> RequestStatus {
        self.floor = remote.floor;
        self.request_type = remote.request_type;
        self.status = remote.status;
        self.acknowledged_by = remote.acknowledged_by.clone();

        self.status
    }

    pub fn update_acknowledgements(&mut self, peers: &Vec<String>, remote_ip: String) -> RequestStatus {
        let ref mut acknowledged_by = self.acknowledged_by;
        acknowledged_by.push(remote_ip);
        acknowledged_by.sort();
        acknowledged_by.dedup();

        // If all elevators have acknowledged, upgrade the request to active.
        for addr in peers.iter() {
            let ip = addr.split(":").next().unwrap().to_string();
            if !acknowledged_by.contains(&ip) {
                return Pending;
            }
        }

        self.status = Active;
        Active
    }
}
