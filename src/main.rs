#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(mpsc_select)]

extern crate elevator;
use elevator::elevator_driver::elev_io::*;
use elevator::elevator_fsm::elevator_fsm::*;

use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::time::{SystemTime, Duration};
use std::thread;
use std::str::*;
use std::process::Command;
use std::env::args;

use std::sync::mpsc::channel;
extern crate chrono;
extern crate timer;
use elevator::request_handler::*;
use elevator::request_handler::request_handler::{RequestType};
use std::rc::Rc;


fn main() {
    let request_transmitter: Rc<request_handler::RequestTransmitter> = Rc::new(
        request_handler::RequestTransmitter::new()
    );
    let mut elevator = Elevator::new(request_transmitter.clone());
    println!("e-elev");
    let ref peer_rx = request_transmitter.peer_receiver;
    let ref request_rx = request_transmitter.bcast_receiver;

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

        if elevator.timer.timeout() { elevator.event_doors_should_close(); }

        let (timer_tx, timer_rx) = channel::<()>();
        let timer = timer::Timer::new();
        let timer_guard = timer.schedule_repeating(chrono::Duration::milliseconds(75), move ||{
            timer_tx.send(()).unwrap();
        });

        timer_guard.ignore();

        select! {
            update_msg = peer_rx.recv() => {
                let update = update_msg.unwrap();
                println!("{}", update);
                elevator.request_handler.handle_peer_update(update);

            },
            bcast_msg = request_rx.recv() => {
                let (message, ip) = bcast_msg.unwrap();
                //println!("Got bcast_msg: {:?}", message);

                let result = elevator.request_handler.merge_incoming_request(&message, ip);

                let button: Button = match (message.floor, message.request_type) {
                    (floor, RequestType::CallUp) => Button::CallUp(Floor::At(floor)),
                    (floor, RequestType::CallDown) => Button::CallDown(Floor::At(floor)),
                    (floor, RequestType::Internal) => Button::Internal(Floor::At(floor)),
                };

                if let Some(Light::On) = result {
                    elevator.event_button_light(button, Light::On);
                } else if let Some(Light::Off) = result {
                    elevator.event_button_light(button, Light::Off);
                }
            },
            _ = timer_rx.recv() => {
                elevator.request_handler.announce_all_requests();
                //elevator.request_handler.print();
            }
        }
    }

}
