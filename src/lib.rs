#![feature(mpsc_select)]
#![feature(type_ascription)]

#[macro_use]
extern crate lazy_static;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate net2;
extern crate rand;
extern crate chrono;
extern crate timer;

pub mod elevator_driver;
pub mod elevator_fsm;
pub mod request_handler;
pub mod network;
pub mod elevator_timer;
