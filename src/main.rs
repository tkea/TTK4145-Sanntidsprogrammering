#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate elevator;

use elevator::elevator_driver::elev_io::*;

// ElevatorData...
struct Elevator {
    io: ElevIo,
    current_direction: MotorDir,
    order: Option<Floor>,
    state: State,
}

impl Elevator {
    fn new() -> Self {
        let elevator_io = ElevIo::new().expect("Init of HW failed");
        elevator_io.set_motor_dir(MotorDir::Down);
        let elevator = Elevator {
            io: elevator_io,
            current_direction: MotorDir::Stop,
            order: None,
            state: State::Idle,
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

    // Transition Idle => Idle
    fn change_direction(&mut self) {
        let new_direction = match self.current_direction {
            MotorDir::Up => MotorDir::Down,
            MotorDir::Down => MotorDir::Up,
            _ => self.current_direction,
        };

        self.io.set_motor_dir(new_direction).unwrap();
        self.current_direction = new_direction;
    }

    fn is_floor_ordered(&self) -> bool {
        return true;
    }
}

// State Machine
enum State {
    Idle,
    Running,
    DoorOpen
}


// State: Idle
fn event_idle(floor: Floor) {

}

fn main() {
    let elevator = Elevator::new();

    loop {


        /*if let Signal::High = elevator.get_stop_signal()
                                     .expect("Get StopSignal failed") {
            elevator.set_motor_dir(MotorDir::Stop)
                   .expect("Set MotorDir failed");
            return;
        } */
    }
}
