#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(mpsc_select)]

extern crate chrono;
extern crate timer;

extern crate elevator;
use elevator::elevator_driver::elev_io::*;
use elevator::elevator_fsm::elevator_fsm::*;

use std::sync::mpsc::channel;
use elevator::request_handler::request_transmitter::*;
use elevator::request_handler::request_transmitter::BroadcastMessage;
use std::rc::Rc;

use std::env;
use std::process::Command;

fn spawn_backup() {
    let args = env::args();
    if args.len() != 1 {
        let mut child = Command::new("cargo")
                            .arg("run -- main")
                            .spawn()
                            .unwrap();

        println!("im a backup");
        child.wait().unwrap();
    }
}

fn main() {
    let request_transmitter: Rc<RequestTransmitter> = Rc::new(
        RequestTransmitter::new()
    );
    let mut elevator = Elevator::new(request_transmitter.clone());

    let ref peer_rx = request_transmitter.peer_receiver;
    let ref request_rx = request_transmitter.bcast_receiver;

    loop {

        if let Floor::At(_) = elevator.io.get_floor_signal().unwrap() {
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

        if elevator.door_timer.timeout() {
            elevator.event_doors_should_close();
        }

        if elevator.stuck_timer.timeout() {
            elevator.event_stuck();
            panic!("Elevator is stuck.");
        }

        let (timer_tx, timer_rx) = channel::<()>();

        let timer = timer::Timer::new();
        let timer_guard = timer.schedule_repeating(chrono::Duration::milliseconds(150), move|| {
            timer_tx.send(());
        });
        timer_guard.ignore();

        select! {
            update_msg = peer_rx.recv() => {
                let update = update_msg.unwrap();
                elevator.request_handler.handle_peer_update(update);
            },
            bcast_msg = request_rx.recv() => {
                let (message, remote_ip) = bcast_msg.unwrap();

                match message {
                    BroadcastMessage::RequestMessage(request) => {
                        elevator.event_request_message(&request, remote_ip);
                    },
                    BroadcastMessage::Position(floor) => {
                        elevator.event_position_message(remote_ip, floor);
                    },
                }
            },
            _ = timer_rx.recv() => {
                request_transmitter.bcast_sender.send(BroadcastMessage::Position(elevator.current_floor));
                elevator.request_handler.announce_all_requests();
            }
        }
    }

}
