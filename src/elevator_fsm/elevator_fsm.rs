#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

use elevator_driver::elev_io::*;
use request_handler::request_handler::*;
use door_timer::door_timer::*;


enum State {
    Idle,
    Running,
    DoorOpen
}


pub struct Elevator {
    pub io: ElevIo,
    current_direction: MotorDir,
    state: State,
    pub request_handler: RequestHandler,
    pub timer: Timer,
}


impl Elevator {

    pub fn new() -> Self {
        let elevator_io = ElevIo::new().expect("Init of HW failed");
        let request_handler = RequestHandler::new();
        let timer = Timer::new();

        elevator_io.set_motor_dir(MotorDir::Down);

        let elevator = Elevator {
            io: elevator_io,
            current_direction: MotorDir::Down,
            state: State::Idle,
            request_handler: request_handler,
            timer: timer,
        };

        return elevator;
    }


//////////////////////////////////// ELEVATOR FUNCTIONS //////////////////////////////////////////

    fn get_current_floor(&self) -> Floor {
        return self.io.get_floor_signal().unwrap();
    }


    fn stop_and_open_doors(&mut self) {
        self.io.set_motor_dir(MotorDir::Stop).unwrap();
        self.io.set_door_light(Light::On).unwrap();
        self.timer.start();
    }


    fn clear_lights_at_floor(&mut self, floor: usize){
        let internal_button = Button::Internal(Floor::At(floor));
        let external_button = match self.current_direction {
            MotorDir::Up => Button::CallUp(Floor::At(floor)),
            MotorDir::Down => Button::CallDown(Floor::At(floor)),
            _ => return
        };

        self.io.set_button_light(internal_button, Light::Off);
        self.io.set_button_light(external_button, Light::Off);
    }


    fn close_doors(&mut self) {
        let current_floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return,
        };

        self.request_handler.announce_requests_cleared(current_floor, self.current_direction);
        self.clear_lights_at_floor(current_floor);
        self.io.set_door_light(Light::Off).unwrap();
    }


    fn set_direction(&mut self) {
        let current_floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return,
        };

        // orders in same direction, so continue
        if self.request_handler.should_continue(current_floor, self.current_direction) {
            self.io.set_motor_dir(self.current_direction);
            return;
        }

        // orders in oppsite direction, so change direction
        if self.request_handler.should_change_direction(current_floor, self.current_direction) {
            let opposite_direction = match self.current_direction {
                MotorDir::Down => MotorDir::Up,
                _ => MotorDir::Down
            };

            self.io.set_motor_dir(opposite_direction);
            self.current_direction = opposite_direction;
            return;
        }

        // no orders in any direction, so stop
        self.io.set_motor_dir(MotorDir::Stop);
    }


    fn set_floor_lights(&self) {
        self.io.set_floor_light(self.io.get_floor_signal().unwrap());
    }


/////////////////////////////////////// FSM EVENTS ///////////////////////////////////////////////

    pub fn event_running(&mut self) {
        if let State::Idle = self.state {
            self.state = State::Running;
        }
    }


    pub fn event_at_floor(&mut self) {
        if let State::Running = self.state {
            self.state = State::Idle;
            self.set_floor_lights();
        }

        if let State::Idle = self.state {
            let floor = match self.get_current_floor() {
                Floor::At(floor) => floor,
                Floor::Between => return,
            };
            if self.request_handler.should_stop(floor, self.current_direction) {
                self.state = State::DoorOpen;
                self.stop_and_open_doors();
            } else {
                self.set_direction();
            }
        }
    }


    pub fn event_new_floor_order(&mut self, button: Button){
        self.request_handler.announce_new_request(&button);
        //self.io.set_button_light(button, Light::On);
    }


    pub fn event_doors_should_close(&mut self) {
        if let State::DoorOpen = self.state {
            self.state = State::Idle;
            self.close_doors();
        }
    }
}
