#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(type_ascription)]

use rand;
use rand::Rng;

use std::rc::Rc;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};

use elevator_driver::elev_io::{N_FLOORS, Floor, Button, MotorDir, Light};

use network::localip::get_localip;
use network::peer::{PeerTransmitter, PeerReceiver, PeerUpdate};
use network::bcast::{BcastTransmitter, BcastReceiver};

use request_handler::request::*;
use request_handler::request::RequestStatus::*;
use request_handler::request_transmitter::*;


pub struct RequestHandler {
    pub requests: Vec<Vec<Request>>,
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

    pub fn get_internal_requests(&self) -> Vec<Request> {
        self.requests[RequestType::Internal as usize].clone()
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

    fn announce_request(&mut self, request: Request) {
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

    pub fn announce_requests_cleared(&mut self, floor: usize, direction: MotorDir) {
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

    pub fn announce_all_requests(&mut self) {
        let call_up = &self.requests[RequestType::CallUp as usize];
        let call_down = &self.requests[RequestType::CallDown as usize];

        for request in call_up.iter().chain(call_down.iter()) {
            self.request_transmitter.announce_request(request.clone());
        }
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

        let requests = i_iter.chain(u_iter).chain(d_iter);

        requests
            .filter(|request| self.request_is_ordered(&request))
            .filter(|request| self.request_is_assigned_locally(&request, floor))
            .count() > 0
    }

    fn request_is_ordered(&self, request: &Request) -> bool {
        if let Active = request.status {
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

        let local_ip = &get_localip().unwrap().to_string();
        let mut min_peer_ip = &get_localip().unwrap().to_string();
        let mut min_peer_cost = 2*N_FLOORS;

        for (peer, position) in &self.peer_positions {
            if peer != local_ip {
                let cost = self.calculate_cost(&request, *position);
                if cost < min_peer_cost {
                    min_peer_ip = &peer;
                    min_peer_cost = cost
                }
            }
        }

        if local_cost == min_peer_cost {
            let ip_to_cost = |ip: &IP| {
                ip
                    .split(":").next().unwrap()
                    .split(".").skip(3).take(1).next().unwrap()
                    .to_string()
            };
            let local: i32 = str::parse(&ip_to_cost(local_ip)).unwrap();
            let remote: i32 = str::parse(&ip_to_cost(min_peer_ip)).unwrap();

            return local <= remote;
        }

        local_cost <= min_peer_cost
    }
}
