#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#![feature(mpsc_select)]

extern crate elevator;
use elevator::elevator_driver::elev_io::*;
use elevator::elevator_fsm::elevator_fsm::*;

use std::sync::mpsc::channel;
extern crate chrono;
extern crate timer;
use elevator::request_handler::*;
use elevator::request_handler::request_handler::BroadcastMessage;
use std::rc::Rc;

use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::time::{SystemTime, Duration};
use std::thread;
use std::str::*;
use std::process::Command;
use std::env::args;
const BACKUP_FILE: &'static str = "backup_data.txt";
const TIMEOUT_MS: u64 = 3000;
const PERIOD_MS: u64 = 1000;


fn main() {
    println!("I'm backup");
    let mut file = OpenOptions::new().read(true).write(true).create(true).open(BACKUP_FILE).unwrap();
    while
    {SystemTime::now().duration_since(file.metadata().unwrap().modified().unwrap()).unwrap() <= Duration::from_millis(TIMEOUT_MS)}
    {}

    let request_transmitter: Rc<request_handler::RequestTransmitter> = Rc::new(
        request_handler::RequestTransmitter::new()
    );
    let mut elevator = Elevator::new(request_transmitter.clone());

    let mut backup = String::new();
    file.read_to_string(&mut backup);
    elevator.request_handler.save_internal_orders(backup);

    println!("Spawning the backup");
    let backup_spawning_command = Command::new("gnome-terminal").arg("-x").arg(args().nth(0).unwrap()).spawn();
    println!("I'm the primary now");

    // writes to BACKUP_FILE every PERIOD_MS
    thread::spawn(move || {
        loop {
            file.set_len(0);
            file.seek(io::SeekFrom::Start(0));
            file.write_fmt(format_args!("Heihei")); //elevator.request_handler.back_it_up()
            thread::sleep(Duration::from_millis(PERIOD_MS));
        }
    });









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

        if elevator.door_timer.timeout() {
            elevator.event_doors_should_close();
        }

        if elevator.stuck_timer.timeout() {
            elevator.event_stuck();
            panic!("Elevator is stuck.");
        }

        let (timer_tx, timer_rx) = channel::<()>();

        let timer = timer::Timer::new();
        let timer_guard = timer.schedule_repeating(chrono::Duration::milliseconds(300), move|| {
            timer_tx.send(());
        });


        timer_guard.ignore();

        select! {
            update_msg = peer_rx.recv() => {
                let update = update_msg.unwrap();
                println!("{}", update);
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
