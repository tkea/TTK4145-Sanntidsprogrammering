#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

use std::rc::Rc;
use elevator_driver::elev_io::*;
use request_handler::request::*;
use request_handler::request_transmitter::*;
use request_handler::request_handler::*;
use elevator_timer::elevator_timer::*;

enum State {
    Idle,
    Running,
    DoorOpen
}


pub struct Elevator {
    pub io: ElevIo,
    pub current_floor: usize,
    current_direction: MotorDir,
    state: State,
    pub request_handler: RequestHandler,
    pub door_timer: Timer,
    pub stuck_timer: Timer,
}


impl Elevator {
    pub fn new(request_transmitter: Rc<RequestTransmitter>) -> Self {
        let elevator_io = ElevIo::new().expect("Init of HW failed");
        let request_handler = RequestHandler::new(request_transmitter);
        let door_timer = Timer::new(2);
        let stuck_timer = Timer::new(5);

        let current_floor = match elevator_io.get_floor_signal().unwrap() {
            Floor::At(floor) => floor,
            Floor::Between => unreachable!(),
        };

        elevator_io.set_motor_dir(MotorDir::Down);

        let elevator = Elevator {
            io: elevator_io,
            current_floor: current_floor,
            current_direction: MotorDir::Down,
            state: State::Idle,
            request_handler: request_handler,
            door_timer: door_timer,
            stuck_timer: stuck_timer,
        };

        return elevator;
    }

    fn get_current_floor(&self) -> Floor {
        return self.io.get_floor_signal().unwrap();
    }


    fn stop_and_open_doors(&mut self) {
        self.state = State::DoorOpen;

        let current_floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return,
        };

        self.request_handler.announce_requests_cleared(current_floor, self.current_direction);

        self.io.set_motor_dir(MotorDir::Stop).unwrap();
        self.io.set_door_light(Light::On).unwrap();
        self.clear_lights_at_floor(current_floor);
        self.door_timer.start();
    }


    fn clear_lights_at_floor(&mut self, floor: usize){
        let internal_button = Button::Internal(Floor::At(floor));
        let external_button = match self.current_direction {
            MotorDir::Up => Button::CallUp(Floor::At(floor)),
            MotorDir::Down => Button::CallDown(Floor::At(floor)),
            _ => unreachable!()
        };

        self.io.set_button_light(internal_button, Light::Off);
        self.io.set_button_light(external_button, Light::Off);
    }


    fn close_doors(&mut self) {
        self.io.set_door_light(Light::Off).unwrap();
    }


    fn set_direction(&mut self) {
        let current_floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return,
        };

        if self.request_handler.should_continue(current_floor, self.current_direction) {
            // orders in same direction, so continue
            self.io.set_motor_dir(self.current_direction);
            return;
        }

        if self.request_handler.should_change_direction(current_floor, self.current_direction) {
            // orders in oppsite direction, so change direction
            let opposite_direction = match self.current_direction {
                MotorDir::Down  => MotorDir::Up,
                MotorDir::Up    => MotorDir::Down,
                _               => unreachable!(),
            };

            self.current_direction = opposite_direction;
            return;
        }

        // no orders in any direction, so stop
        self.io.set_motor_dir(MotorDir::Stop);
    }


    fn set_floor_lights(&self) {
        self.io.set_floor_light(self.io.get_floor_signal().unwrap());
    }


    pub fn event_running(&mut self) {
        if let State::Idle = self.state {
            self.state = State::Running;
        }
    }


    pub fn event_at_floor(&mut self) {
        self.stuck_timer.start();

        if let State::Running = self.state {
            self.state = State::Idle;
        }

        if let State::Idle = self.state {
            self.set_floor_lights();

            let floor = match self.get_current_floor() {
                Floor::At(floor) => floor,
                Floor::Between => return,
            };

            self.current_floor = floor;

            if self.request_handler.should_stop(floor, self.current_direction) {
                self.state = State::DoorOpen;
                self.stop_and_open_doors();
            } else {
                self.set_direction();
            }
        }
    }


    pub fn event_new_floor_order(&mut self, button: Button){
        if let Button::Internal(Floor::At(floor)) = button {
            let internal = RequestType::Internal as usize;
            self.request_handler.requests[internal][floor] = Request {
                floor: floor,
                request_type: RequestType::Internal,
                status:RequestStatus::Active,
                ..Request::default()
            };
            self.event_update_button_light(button, Light::On);
        } else {
            self.request_handler.announce_new_request(&button);
        }

    }


    pub fn event_doors_should_close(&mut self) {
        if let State::DoorOpen = self.state {
            self.state = State::Idle;
            self.close_doors();
        }
    }

    pub fn event_update_button_light(&mut self, button: Button, mode: Light) {
        self.io.set_button_light(button, mode);
    }

    pub fn event_stuck(&self) {
        self.io.set_motor_dir(MotorDir::Stop);
    }

    pub fn event_request_message(&mut self, message: &Request, remote_ip: String) {
        let result = self.request_handler.merge_incoming_request(&message, remote_ip);

        let button: Button = match (message.floor, message.request_type) {
            (floor, RequestType::CallUp) => Button::CallUp(Floor::At(floor)),
            (floor, RequestType::CallDown) => Button::CallDown(Floor::At(floor)),
            (floor, RequestType::Internal) => Button::Internal(Floor::At(floor)),
        };

        if let Some(Light::On) = result {
            self.event_update_button_light(button, Light::On);
        } else if let Some(Light::Off) = result {
            self.event_update_button_light(button, Light::Off);
        }
    }

    pub fn event_position_message(&mut self, remote_ip: String, position: usize) {
        self.request_handler.handle_position_update(remote_ip, position);
    }


}
