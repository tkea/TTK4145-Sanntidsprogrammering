#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]


extern crate timer;
extern crate chrono;

use elevator_driver::elev_io::*;


// State
enum State {
    Idle,
    Running,
    DoorOpen
}


// ElevatorData...
pub struct Elevator {
    pub io: ElevIo,
    current_direction: MotorDir,
    state: State,
    orders_up: [bool; N_FLOORS],
    orders_down: [bool; N_FLOORS],
    orders_internal: [bool; N_FLOORS],
}


impl Elevator {
    pub fn new() -> Self {
        let elevator_io = ElevIo::new().expect("Init of HW failed");
        elevator_io.set_motor_dir(MotorDir::Down);
        let elevator = Elevator {
            io: elevator_io,
            current_direction: MotorDir::Down,
            state: State::Idle,
            orders_up: [false; N_FLOORS],
            orders_down: [false; N_FLOORS],
            orders_internal: [false; N_FLOORS],
        };

        return elevator;
    }

    fn get_current_floor(&self) -> Floor {
        return self.io.get_floor_signal().unwrap();
    }


    // Transition Idle => DoorOpen
    pub fn stop_and_open_doors(&mut self) /*-> Receiver*/ {
        self.state = State::DoorOpen;
        self.io.set_motor_dir(MotorDir::Stop).unwrap();
        self.io.set_door_light(Light::On).unwrap();

        /*let timer = timer::Timer::new();
        let (tx, rx) = channel();
        self._guard = timer.schedule_with_delay(chrono::Duration::seconds(3),
            move|| {
                tx.send(());
            });
        rx*/
        self.state = State::Idle;
        self.close_doors();
    }


    // Transition DoorOpen => Idle
    pub fn close_doors(&mut self) {
        self.clear_orders_here();
        self.io.set_door_light(Light::Off).unwrap();
        self.state = State::Idle;
    }


    pub fn new_floor_order(&mut self, button: Button) {
        match button {
            Button::CallUp(Floor::At(floor)) => self.orders_up[floor] = true,
            Button::CallDown(Floor::At(floor)) => self.orders_down[floor] = true,
            Button::Internal(Floor::At(floor)) => self.orders_internal[floor] = true,
            _ => {}
        }

        self.io.set_button_light(button, Light::On);
    }


    fn clear_orders_here(&mut self) {
        let current_floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return,
        };

        // Clear orders
        self.orders_internal[current_floor] = false;

        match self.current_direction {
            MotorDir::Up => self.orders_up[current_floor] = false,
            MotorDir::Down => self.orders_down[current_floor] = false,
            _ => ()
        }

        // Turn off lights
        let internal_button = Button::Internal(Floor::At(current_floor));
        self.io.set_button_light(internal_button, Light::Off);

        let external_button = match self.current_direction {
            MotorDir::Up => Button::CallUp(Floor::At(current_floor)),
            MotorDir::Down => Button::CallDown(Floor::At(current_floor)),
            _ => return
        };
        self.io.set_button_light(external_button, Light::Off);
    }


    pub fn should_stop(&self) -> bool {
        let floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return false,
        };

        let should_stop = match self.current_direction {
            MotorDir::Up => self.orders_up[floor] || self.orders_internal[floor],
            MotorDir::Down => self.orders_down[floor] || self.orders_internal[floor],
            _ => false
        };

        return should_stop;
    }

    fn orders_in_direction(&self, direction: MotorDir) -> bool {
        let current_floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return true,
        };

        let (lower_bound, num_elements) = match direction {
            MotorDir::Down => (0, current_floor),
            _ => (current_floor+1, N_FLOORS-current_floor+1),
        };

        let internal = self.orders_internal.iter().skip(lower_bound).take(num_elements);
        let up = self.orders_up.iter().skip(lower_bound).take(num_elements);
        let down = self.orders_down.iter().skip(lower_bound).take(num_elements);

        let mut orders = internal.chain(up).chain(down);

        if orders.any(|&floor_ordered| floor_ordered) {
            return true;
        } else {
            return false;
        }
    }

    fn should_continue(&self) -> bool {
        return self.orders_in_direction(self.current_direction);
    }

    fn should_change_direction(&self) -> bool {
        let opposite_direction = match self.current_direction {
            MotorDir::Down => MotorDir::Up,
            _ => MotorDir::Down
        };

        if self.orders_in_direction(opposite_direction) {
            return true;
        }

        // Handle edge case
        // where current_floor == 0 || current_floor == top floor
        let current_floor = match self.get_current_floor() {
            Floor::At(floor) => floor,
            Floor::Between => return true,
        };

        let orders_opposite = match self.current_direction {
            MotorDir::Down => self.orders_up,
            _ => self.orders_down
        };

        let top_floor = N_FLOORS-1;
        
        let is_at_top_or_bottom: bool = match current_floor {
            top_floor   => true,
            0           => true,
            _           => false
        };

        if is_at_top_or_bottom && orders_opposite[current_floor] {
            return true;
        }

        return false;
    }


    // continue, stop or change direction
    pub fn set_direction(&mut self) {
        if self.should_continue() {
            self.io.set_motor_dir(self.current_direction);
            return;
        }

        if self.should_change_direction() {
            let opposite_direction = match self.current_direction {
                MotorDir::Down => MotorDir::Up,
                _ => MotorDir::Down
            };

            self.io.set_motor_dir(opposite_direction);
            self.current_direction = opposite_direction;
            return;
        }

        // No orders in any direction, so stop
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


    // State: Idle
    pub fn event_at_floor(&mut self) {
        if let State::Running = self.state {
            self.state = State::Idle;
            self.set_floor_lights();

        }

        if let State::Idle = self.state {
            if self.should_stop() {
                self.state = State::DoorOpen;
                self.stop_and_open_doors();
            } else {
                self.set_direction();
            }
        }
    }


    pub fn event_new_floor_order(&mut self, button: Button){
        self.new_floor_order(button);
    }


    pub fn event_doors_should_close(&mut self) {
        if let State::DoorOpen = self.state {
            self.state = State::Idle;
        }
    }
}
