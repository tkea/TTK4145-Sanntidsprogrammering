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

use elevator_driver::elev_io::{Floor, Button, MotorDir, Light};
use std::rc::Rc;
use self::RequestStatus::*;

const PEER_PORT: u16 = 9877;
const BCAST_PORT: u16 = 9876;

const N_FLOORS: usize = 4;

type IP = String;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BroadcastMessage {
    RequestMessage(Request),
    Position(usize),
}

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
enum RequestStatus {
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
    status: RequestStatus,
    acknowledged_by: Vec<IP>,
}

impl Request {
    fn move_to_active(&mut self) -> RequestStatus {
        self.status = Active;
        Active
    }

    fn move_to_pending(&mut self) -> RequestStatus {
        self.status = Pending;
        Pending
    }

    fn move_to_inactive(&mut self) -> RequestStatus {
        self.status = Inactive;
        Inactive
    }

    fn handle_unknown_local(&mut self, remote: &Request) -> RequestStatus {
        self.floor = remote.floor;
        self.request_type = remote.request_type;
        self.status = remote.status;
        self.acknowledged_by = remote.acknowledged_by.clone();

        self.status
    }

    fn update_acknowledgements(&mut self, peers: &Vec<String>, remote_ip: String) -> RequestStatus {
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

pub struct RequestHandler {
    requests: Vec<Vec<Request>>,
    peers: Vec<String>,
    peer_positions: HashMap<IP, usize>,
    request_transmitter: Rc<RequestTransmitter>,
}

impl RequestHandler {
    pub fn new(request_transmitter: Rc<RequestTransmitter>) -> Self {
        // Initializing the requests array is complicated since RequestHandler does not
        // implement Copy.
        let mut requests = vec!(vec!(), vec!(), vec!());

        for &t in [RequestType::CallDown, RequestType::CallUp, RequestType::Internal].iter() {
            let mut rs = vec!();
            for floor in 0..N_FLOORS {
                let request = Request {floor: floor, request_type: t, ..Request::default()};
                rs.push(request);
            }
            requests[t: RequestType as usize] = rs;
        }

        RequestHandler {
            requests: requests,
            peers: Vec::new(),
            peer_positions: HashMap::new(),
            request_transmitter: request_transmitter,
        }
    }

    pub fn handle_peer_update(&mut self, peers: PeerUpdate<String>) {
        self.peers = peers.peers;

        for peer_addr in peers.lost {
            let ip = peer_addr.split(":").next().unwrap().to_string();
            self.peer_positions.remove(&ip);
        }

        if let Some(peer_addr) = peers.new {
            let ip = peer_addr.split(":").next().unwrap().to_string();
            self.peer_positions.insert(ip, 0);
        }
    }

    pub fn handle_position_update(&mut self, remote_ip: IP, position: usize) {
        self.peer_positions.insert(remote_ip, position);
    }

    fn get_local_request(&mut self, remote_request: &Request) -> &mut Request {
        let floor = remote_request.floor;
        let request_type = remote_request.request_type as usize;

        &mut self.requests[request_type][floor]
    }

    pub fn merge_incoming_request(&mut self, remote_request: &Request, remote_ip: IP) -> Option<Light> {
        let peers = self.peers.clone();

        let ref mut local_request = self.get_local_request(&remote_request);

        let local_status = local_request.status.clone();
        let remote_status = remote_request.status;

        let new_status = match (local_status, remote_status) {
            (Active, Inactive)  => local_request.move_to_inactive(),
            (Inactive, Pending) => local_request.move_to_pending(),
            (Pending, Active)   => local_request.move_to_active(),
            (Pending, Pending)  => local_request.update_acknowledgements(&peers, remote_ip),
            (Unknown, _)        => local_request.handle_unknown_local(&remote_request),
            _                   => return None,
        };

        // The return value determines if the button light needs to be turned on.
        match (local_status, new_status) {
            (Pending, Active)   => return Some(Light::On),
            (Active, Inactive)  => return Some(Light::Off),
            _                   => return None,
        }
    }

    fn announce_request(&self, request: Request) {
        self.request_transmitter.announce_request(request);
    }

    pub fn announce_new_request(&mut self, button: &Button) {
        let (request_type, floor) = match button {
            &Button::Internal(Floor::At(floor)) => (RequestType::Internal,  floor),
            &Button::CallUp(Floor::At(floor))   => (RequestType::CallUp,    floor),
            &Button::CallDown(Floor::At(floor)) => (RequestType::CallDown,  floor),
            _                                   => return,
        };

        let request = Request {
            floor: floor,
            request_type: request_type,
            status: Pending,
            ..Request::default()
        };

        self.announce_request(request);
    }

    pub fn announce_requests_cleared(&self, floor: usize, direction: MotorDir) {
        // Clears the requests at a floor.
        let hall_request_type = match direction {
            MotorDir::Up    => RequestType::CallUp,
            MotorDir::Down  => RequestType::CallDown,
            _               => unreachable!(),
        };

        let hall_request = Request {
            floor: floor,
            request_type: hall_request_type,
            status: Inactive,
            ..Request::default()
        };

        let internal_request = Request {
            floor: floor,
            request_type: RequestType::Internal,
            status: Inactive,
            ..Request::default()
        };

        self.announce_request(internal_request);
        self.announce_request(hall_request);
    }

    pub fn should_continue(&self, floor: usize, direction: MotorDir) -> bool {
        self.requests_in_direction(floor, direction)
    }

    pub fn should_change_direction(&self, floor: usize, direction: MotorDir) -> bool {
        let opposite_direction = match direction {
            MotorDir::Down  => MotorDir::Up,
            MotorDir::Up    => MotorDir::Down,
            _               => unreachable!(),
        };

        // Check opposite order on same floor
        let request_opposite = match direction {
            MotorDir::Up    => &self.requests[RequestType::CallDown as usize][floor],
            MotorDir::Down  => &self.requests[RequestType::CallUp as usize][floor],
            _               => unreachable!(),
        };

        if let Active = request_opposite.status {
            return true;
        }


        // Check orders below
        if self.requests_in_direction(floor, opposite_direction) {
            return true;
        }

        false
    }

    pub fn should_stop(&self, floor: usize, direction: MotorDir) -> bool {
        let internal_requests = &self.requests[RequestType::Internal as usize];

        let hall_requests = match direction {
            MotorDir::Up    => &self.requests[RequestType::CallUp as usize],
            MotorDir::Down  => &self.requests[RequestType::CallDown as usize],
            _               => unreachable!(),
        };

        let internal_is_requested = match internal_requests[floor].status {
            Active  => true,
            _       => false,
        };

        let hall_is_requested = match hall_requests[floor].status {
            Active  => true,
            _       => false,
        };

        let should_stop = internal_is_requested || hall_is_requested;

        should_stop
    }

    fn requests_in_direction(&self, floor: usize, direction: MotorDir) -> bool {
        let requests_internal   = &self.requests[RequestType::Internal as usize];
        let requests_up         = &self.requests[RequestType::CallUp as usize];
        let requests_down       = &self.requests[RequestType::CallDown as usize];

        let (lower_bound, num_elements) = match direction {
            MotorDir::Down  => (0, floor),
            MotorDir::Up    => (floor+1, N_FLOORS-floor+1),
            _               => unreachable!(),
        };

        let i_iter = requests_internal .iter().skip(lower_bound).take(num_elements);
        let u_iter = requests_up       .iter().skip(lower_bound).take(num_elements);
        let d_iter = requests_down     .iter().skip(lower_bound).take(num_elements);

        let mut requests = i_iter.chain(u_iter).chain(d_iter);

        //requests.any(|request| self.request_is_ordered(&request))
        requests
            .filter(|request| self.request_is_ordered(&request))
            .filter(|request| self.request_is_assigned_locally(&request, floor))
            .count() > 0
    }

    fn request_is_ordered(&self, request: &Request) -> bool {
        if let Active = request.status {
            println!("request {:?}", request);
            return true;
        } else {
            return false;
        }
    }

    fn calculate_cost(&self, request: &Request, position: usize) -> usize {
        let cost = (request.floor as isize) - (position as isize);

        if cost < 0 {
            return (cost*-1) as usize;
        }

        return cost as usize;
    }

    fn request_is_assigned_locally(&self, request: &Request, local_position: usize) -> bool {
        let local_cost = self.calculate_cost(&request, local_position);


        let mut min_peer_cost = 2*N_FLOORS;

        for (_, position) in &self.peer_positions {
            let cost = self.calculate_cost(&request, *position);
            if cost < min_peer_cost {
                min_peer_cost = cost;
            }
        }
        println!("local cost {:?} minpeer {:?}", local_cost, min_peer_cost);
        local_cost <= min_peer_cost
    }

    pub fn announce_all_requests(&self) {
        for t in self.requests.iter() {
            for request in t.iter() {
                self.announce_request(request.clone());
            }
        }
    }
}

// RQT
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
