#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate elevator;
extern crate timer;
extern crate chrono;

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
            current_direction: MotorDir::Down,
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
    fn stop_and_open_doors(&mut self) /*-> Receiver*/ {
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

        self.close_doors();
    }

    // Transition DoorOpen => Idle
    fn close_doors(&mut self) {
        self.clear_orders_here();
        self.io.set_door_light(Light::Off).unwrap();
        self.state = State::Idle;
    }

    fn new_floor_order(&mut self, button: Button) {
        match button {
            Button::CallUp(Floor::At(floor)) => self.orders_up[floor] = true,
            Button::CallDown(Floor::At(floor)) => self.orders_down[floor] = true,
            Button::Internal(Floor::At(floor)) => self.orders_internal[floor] = true,
            _ => {}
        }

        self.io.set_button_light(button, Light::On);
    }

    fn clear_orders_here(&mut self) {
        let current_floor = match self.io.get_floor_signal().unwrap() {
            Floor::Between => return,
            Floor::At(floor) => floor,
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

    fn should_stop(&self) -> bool {
        let floor = match self.io.get_floor_signal().unwrap() {
            Floor::Between => return false,
            Floor::At(floor) => floor,
        };

        let should_stop = match self.current_direction {
            MotorDir::Up => self.orders_up[floor] || self.orders_internal[floor],
            MotorDir::Down => self.orders_down[floor] || self.orders_internal[floor],
            _ => false
        };

        return should_stop;
    }

    fn orders_in_direction(&self, direction: MotorDir) -> bool {
        let current_floor = match self.io.get_floor_signal().unwrap() {
            Floor::Between => return true,
            Floor::At(floor) => floor,
        };

        let (lower_bound, upper_bound) = match direction {
            MotorDir::Down => (0, current_floor),
            _ => (current_floor+1, N_FLOORS),
        };

        for floor in lower_bound..upper_bound {
            if self.orders_internal[floor] || self.orders_up[floor] || self.orders_down[floor] {
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

    fn set_floor_lights(&self) {
        self.io.set_floor_light(self.io.get_floor_signal().unwrap());
    }
}


// State: Idle
fn event_at_floor(elevator: &mut Elevator) {
    elevator.set_floor_lights();

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
    //event_new_floor_order(&mut elevator, Button::Internal(Floor::At(2)));

    loop {
        if let Floor::At(floor) = elevator.io.get_floor_signal().unwrap() {
            event_at_floor(&mut elevator);
        }

        let TOP_FLOOR = N_FLOORS-1;
        for floor in 0..N_FLOORS {
            // Buttons at current floor
            let button_call_up = Button::CallUp(Floor::At(floor));
            let button_call_down = Button::CallDown(Floor::At(floor));
            let button_internal = Button::Internal(Floor::At(floor));

            if floor != TOP_FLOOR {
                if let Signal::High = elevator.io.get_button_signal(button_call_up).unwrap() {
                    event_new_floor_order(&mut elevator, button_call_up);
                }
            }

            if floor != 0 {
                if let Signal::High = elevator.io.get_button_signal(button_call_down).unwrap() {
                    event_new_floor_order(&mut elevator, button_call_down);
                }
            }

            if let Signal::High = elevator.io.get_button_signal(button_internal).unwrap() {
                event_new_floor_order(&mut elevator, button_internal);
            }
        }

        if let Signal::High = elevator.io.get_stop_signal()
                                     .expect("Get StopSignal failed") {
            elevator.io.set_motor_dir(MotorDir::Stop)
                   .expect("Set MotorDir failed");
            return;
        }
    }
}
