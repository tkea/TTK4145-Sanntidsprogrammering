#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate elevator;
use elevator::elevator_driver::elev_io::*;
use elevator::elevator_fsm::elevator_fsm::*;

// using Kjetil Kjeka's example for process pair, modified to suit our problem
use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::time::{SystemTime, Duration};
use std::thread;
use std::str::*;
use std::process::Command;
use std::env::args;
const FILEPATH: &'static str = "backup_data.txt";
const TIMEOUT_MS: u64 = 3000;
const PERIOD_MS: u64 = 1000;



fn main() {
    // backup waits for primary to die
    println!("I'm backup");
    let mut file = OpenOptions::new().read(true).write(true).create(true).open(FILEPATH).unwrap();
    while {SystemTime::now().duration_since(file.metadata().unwrap().modified().unwrap()).unwrap() <= Duration::from_millis(TIMEOUT_MS)} {}
    println!("Can't find primary");
    let mut file_source = FILEPATH;
    println!("The source is: \"{}\"", file_source);
                                                                                                    // TODO read the backup data from the txt-file
    println!("Spawning the backup");
    let backup_spawning_command = Command::new("gnome-terminal").arg("-x").arg(args().nth(0).unwrap()).spawn();
    println!("I'm the primary now");

    // backup thread
    thread::spawn(move || {
        loop {
            file.set_len(0);
            file.seek(io::SeekFrom::Start(0));
            file.write_fmt(format_args!("Her skal det skrives inn heisbestillinger."));             // TODO save the backup data to the txt-file
            thread::sleep(Duration::from_millis(PERIOD_MS));
        }
    });

    // primary loop
    let mut elevator = Elevator::new();

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

    }

}
