#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate elevator;

use std::thread;
use elevator::elevator_driver::elev_io::*;
use elevator::elevator_fsm::elevator_fsm::*;

fn main() {
    let mut elevator = Elevator::new();
    //event_new_floor_order(&mut elevator, Button::Internal(Floor::At(2)));
    loop {
        if let Floor::At(floor) = elevator.io.get_floor_signal().unwrap() {
            elevator.event_at_floor();
        } else {
            elevator.event_running();
        }

        let TOP_FLOOR = N_FLOORS-1;
        for floor in 0..N_FLOORS {
            // Buttons at current floor
            let button_call_up = Button::CallUp(Floor::At(floor));
            let button_call_down = Button::CallDown(Floor::At(floor));
            let button_internal = Button::Internal(Floor::At(floor));

            if floor != TOP_FLOOR {
                if let Signal::High = elevator.io.get_button_signal(button_call_up).unwrap() {
                    elevator.event_new_floor_order(button_call_up);
                }
            }

            if floor != 0 {
                if let Signal::High = elevator.io.get_button_signal(button_call_down).unwrap() {
                    elevator.event_new_floor_order(button_call_down);
                }
            }

            if let Signal::High = elevator.io.get_button_signal(button_internal).unwrap() {
                elevator.event_new_floor_order(button_internal);
            }
        }

        if let Signal::High = elevator.io.get_stop_signal()
                                     .expect("Get StopSignal failed") {
            elevator.io.set_motor_dir(MotorDir::Stop)
                   .expect("Set MotorDir failed");
            return;
        }

        if elevator.timer_timeout() { elevator.event_doors_should_close(); }

    }
}
