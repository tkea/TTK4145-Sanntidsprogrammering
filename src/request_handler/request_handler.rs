#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(type_ascription)]

#![macro_use]
use serde_derive;
use rand;

use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::{channel, Sender, Receiver};

use rand::Rng;

use network::localip::get_localip;
use network::peer::{PeerTransmitter, PeerReceiver, PeerUpdate};
use network::bcast::{BcastTransmitter, BcastReceiver};

use elevator_driver::elev_io::{Floor, Button, MotorDir};
use std::rc::Rc;
use self::RequestStatus::*;

const PEER_PORT: u16 = 9877;
const BCAST_PORT: u16 = 9876;

const N_FLOORS: usize = 4;

type IP = String;
type ElevatorID = IP;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum RequestType {
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
    floor: usize,
    request_type: RequestType,
    status: RequestStatus,
    acknowledged_by: Vec<ElevatorID>,
}

impl Request {
    fn move_to_active(&mut self) {
        self.status = Active;
    }

    fn move_to_pending(&mut self) {
        self.status = Pending;
    }

    fn move_to_inactive(&mut self) {
        self.status = Inactive;
    }

    fn handle_unknown_local(&mut self, remote: &Request) {
        self.floor = remote.floor;
        self.request_type = remote.request_type;
        self.status = remote.status;
        self.acknowledged_by = remote.acknowledged_by.clone();
    }

    fn update_acknowledgements(&mut self, peers: &Vec<String>, remote_ip: String) {
        let ref mut acknowledged_by = self.acknowledged_by;
        acknowledged_by.push(remote_ip);
        acknowledged_by.sort();
        acknowledged_by.dedup();

        // If all elevators have acknowledged, upgrade the request to active.
        for addr in peers.iter() {
            let ip = addr.split(":").next().unwrap().to_string();
            if !acknowledged_by.contains(&ip) {
                return;
            }
        }

        self.status = Active;
    }
}

pub struct RequestHandler {
    requests: Vec<Vec<Request>>,
    peers: Vec<String>,
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
            request_transmitter: request_transmitter,
        }
    }

    pub fn handle_peer_update(&mut self, peers: PeerUpdate<String>) {
        self.peers = peers.peers;
    }

    fn get_local_request(&mut self, remote_request: &Request) -> &mut Request {
        let floor = remote_request.floor;
        let request_type = remote_request.request_type as usize;

        &mut self.requests[request_type][floor]
    }

    pub fn merge_incoming_request(&mut self, remote_request: &Request, remote_ip: String) {
        let peers = self.peers.clone();

        let ref mut local_request = self.get_local_request(&remote_request);

        let local_status = local_request.status;
        let remote_status = remote_request.status;

        match (local_status, remote_status) {
            (Active, Inactive)  => local_request.move_to_inactive(),
            (Inactive, Pending) => local_request.move_to_pending(),
            (Pending, Active)   => local_request.move_to_active(),
            (Pending, Pending)  => local_request.update_acknowledgements(&peers, remote_ip),
            (Unknown, _)        => local_request.handle_unknown_local(&remote_request),
            _                   => return,
        }
    }

    pub fn print(&self) {
        for req_type in self.requests.iter() {
            for request in req_type.iter() {
                println!("{:?}", request);
            }
        }
    }

    fn announce_request(&self, request: Request) {
        self.request_transmitter.announce_request(request);
    }

    pub fn announce_new_request(&mut self, button: &Button) {
        let (request_type, floor) = match button {
            &Button::Internal(Floor::At(floor))  => (RequestType::Internal,  floor),
            &Button::CallUp(Floor::At(floor))    => (RequestType::CallUp,    floor),
            &Button::CallDown(Floor::At(floor))  => (RequestType::CallDown,  floor),
            _                                   => return,
        };

        let request = Request {
            floor: floor,
            request_type: RequestType::CallUp,
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

        if self.requests_in_direction(floor, opposite_direction) {
            return true;
        }

        // Handle edge case where current_floor == 0 || current_floor == top floor
        let top_floor = N_FLOORS-1;
        let is_at_top_or_bottom = floor == top_floor || floor == 0;

        if is_at_top_or_bottom {
            let request_opposite = match direction {
                MotorDir::Up    => &self.requests[RequestType::CallDown as usize][floor],
                MotorDir::Down  => &self.requests[RequestType::CallUp as usize][floor],
                _               => unreachable!(),
            };

            if let Active = request_opposite.status {
                return true;
            }
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
        let (lower_bound, num_elements) = match direction {
            MotorDir::Down  => (0, floor),
            MotorDir::Up    => (floor+1, N_FLOORS-floor+1),
            _               => unreachable!(),
        };

        let requests_internal   = &self.requests[RequestType::Internal as usize];
        let requests_up         = &self.requests[RequestType::CallUp as usize];
        let requests_down       = &self.requests[RequestType::CallDown as usize];

        let internal    = requests_internal .iter().skip(lower_bound).take(num_elements);
        let up          = requests_up       .iter().skip(lower_bound).take(num_elements);
        let down        = requests_down     .iter().skip(lower_bound).take(num_elements);

        let mut orders = internal.chain(up).chain(down);

        orders.any(|request| match request.status {
            Active  => true,
            _       => false,
        })
    }

    fn calculate_cost(&self) {
        unimplemented!()
    }

    fn local_is_minimal_cost(&self) {
        unimplemented!()
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

fn spawn_bcast_threads(transmit_rx: Receiver<Request>, receive_tx: Sender<(Request, IP)>) {
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
    bcast_sender: Sender<Request>,
    pub bcast_receiver: Receiver<(Request, IP)>,
    pub peer_receiver: Receiver<PeerUpdate<IP>>,
}

impl RequestTransmitter {
    pub fn new() -> Self {
        let (peer_tx, peer_rx) = channel::<PeerUpdate<IP>>();
        spawn_peer_update_threads(peer_tx);

        let (bcast_transmitter_tx, bcast_transmitter_rx) = channel::<Request>();
        let (bcast_receiver_tx, bcast_receiver_rx) = channel::<(Request, IP)>();
        spawn_bcast_threads(bcast_transmitter_rx, bcast_receiver_tx);

        RequestTransmitter {
            bcast_sender: bcast_transmitter_tx,
            bcast_receiver: bcast_receiver_rx,
            peer_receiver: peer_rx,
        }
    }

    pub fn announce_request(&self, request: Request) {
        self.bcast_sender.send(request)
            .expect("Could not announce request");
    }
}



/*
    pub fn local_is_minimal_cost(&self, ordered_floor: usize, floor: usize, direction: MotorDir,
                           other_floors: [usize; 2], other_directions: [MotorDir; 2]) -> bool{      //TODO length of arrays must be NUM_elevators

        let local_cost = self.calculate_cost(ordered_floor, floor, direction);

        let min_extern_cost = 2 * N_FLOORS; // initialized to max-value of the cost function
        for index in 0..2 {                                                                         //TODO length of arrays must be NUM_elevators
            let extern_cost = self.calculate_cost(
                ordered_floor, other_floors[index], other_directions[index]);
            let min_extern_cost = if extern_cost < min_extern_cost {extern_cost} else {2*N_FLOORS};
        }

        if local_cost < min_extern_cost { return true; }
        else if local_cost == min_extern_cost { return true;  }                                     // TODO lowest IP takes the order
        else { return false; }
    }

    pub fn calculate_cost(&self, ordered_floor: usize, floor: usize, direction: MotorDir) -> usize{
        // calculate the distance_cost: difference between ordered_floor and current_floor
        let distance_cost = (ordered_floor as isize) - (floor as isize);
        let distance_cost = if distance_cost < 0 { distance_cost * -1 } else { distance_cost };

        // calculate the direction_cost: N_FLOORS if the order is in the opposite direction
        let elevator_direction = match direction {
            MotorDir::Down => 0,
            _ => 1,
        };
        let order_direction = if (ordered_floor as isize) - (floor as isize) < 0 { 0 } else {1};
        let direction_cost = if elevator_direction == order_direction {0} else {N_FLOORS};

        return (distance_cost as usize) + (direction_cost as usize);
    }
}
*/
