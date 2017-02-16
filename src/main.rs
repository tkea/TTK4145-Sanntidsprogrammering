#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate elevator;

use elevator::elevator_driver::elev_io::*;

// State
enum State {
    Idle,
    Running,
    DoorOpen
}

// ElevatorData...
struct Elevator {
    io: ElevIo,
    current_direction: MotorDir,
    state: State,
    orders_up: [bool; N_FLOORS],
    orders_down: [bool; N_FLOORS],
    orders_internal: [bool; N_FLOORS],
}

impl Elevator {
    fn new() -> Self {
        let elevator_io = ElevIo::new().expect("Init of HW failed");
        elevator_io.set_motor_dir(MotorDir::Down);
        let elevator = Elevator {
            io: elevator_io,
            current_direction: MotorDir::Stop,
            state: State::Idle,
            orders_up: [false; N_FLOORS],
            orders_down: [false; N_FLOORS],
            orders_internal: [false; N_FLOORS],
        };

        return elevator;
    }

    fn set_state(&mut self, state: State) {
        self.state = state;
    }

    // Transition Idle => DoorOpen
    fn stop_and_open_doors(&mut self) {
        self.state = State::DoorOpen;
        self.io.set_motor_dir(MotorDir::Stop).unwrap();
        self.io.set_door_light(Light::On).unwrap();
    }

    // Transition DoorOpen => Idle
    fn close_doors(&mut self) {
        self.state = State::Idle;
        self.io.set_door_light(Light::Off).unwrap();
    }

    fn new_floor_order(&mut self, button: Button){
        match button {
            Button::CallUp(Floor::At(floor)) => self.orders_up[floor] = true,
            Button::CallDown(Floor::At(floor)) => self.orders_down[floor] = true,
            Button::Internal(Floor::At(floor)) => self.orders_internal[floor] = true,
            _ => {}
        }
    }

    fn get_orders_in_current_direction(&self) -> [bool; N_FLOORS] {
        let orders_in_current_direction = match self.current_direction {
            MotorDir::Up => self.orders_up,
            MotorDir::Down => self.orders_down,
            _ => self.orders_up,
        };

        return orders_in_current_direction;
    }

    fn should_stop(&self) -> bool {
        let current_floor = match self.io.get_floor_signal().unwrap() {
            Floor::Between => return false,
            Floor::At(floor) => floor,
        };

        if self.orders_internal[current_floor] {
            return true;
        }

        let orders_in_current_direction = self.get_orders_in_current_direction();

        if orders_in_current_direction[current_floor] {
            return true;
        }

        return false;
    }

    fn orders_in_direction(&self, direction: MotorDir) -> bool {
        let current_floor = match self.io.get_floor_signal().unwrap() {
            Floor::Between => return true,
            Floor::At(floor) => floor,
        };

        let orders_in_current_direction = self.get_orders_in_current_direction();

        let (lower_bound, upper_bound) = match direction {
            MotorDir::Down => (0, current_floor),
            _ => (current_floor, N_FLOORS-1),
        };

        for floor in lower_bound..upper_bound {
            if self.orders_internal[floor] || orders_in_current_direction[floor] {
                return true;
            }
        }

        return false;
    }

    fn set_direction(&mut self) {
        if self.orders_in_direction(self.current_direction) {
            self.io.set_motor_dir(self.current_direction);
            return;
        }

        let opposite_direction = match self.current_direction {
            MotorDir::Down => MotorDir::Up,
            _ => MotorDir::Down
        };

        if self.orders_in_direction(opposite_direction) {
            self.io.set_motor_dir(opposite_direction);
            self.current_direction = opposite_direction; // Update current direction
            return;
        }

        // No orders in any direction, so just stop
        self.io.set_motor_dir(MotorDir::Stop);
    }
}


// State: Idle
fn event_at_floor(elevator: &mut Elevator) {
    if elevator.should_stop() {
        elevator.stop_and_open_doors();
    } else {
        elevator.set_direction();
    }
}

fn event_new_floor_order(elevator: &mut Elevator, button: Button){
    elevator.new_floor_order(button);
}

fn event_door_should_close(elevator: &mut Elevator) {
    elevator.set_direction();
}

fn main() {
    let mut elevator = Elevator::new();

    loop {
        if let Floor::At(floor) = elevator.io.get_floor_signal().unwrap() {
            event_at_floor(&mut elevator);
        }

        if let Signal::High = elevator.io.get_stop_signal()
                                     .expect("Get StopSignal failed") {
            elevator.io.set_motor_dir(MotorDir::Stop)
                   .expect("Set MotorDir failed");
            return;
        }
    }
}
