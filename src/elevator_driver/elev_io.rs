#![cfg_attr(feature="clippy", allow(identity_op))]
#![allow(dead_code)]

use std::io;

use elevator_driver::hw_io::HwIo;

pub struct ElevIo {
    io: HwIo,
}

#[derive(Copy, Clone)]
pub enum Floor {
    At(usize),
    Between,
}
pub const N_FLOORS: usize = 4;
const TOP: usize = N_FLOORS - 1;
const SEC_TOP: usize = N_FLOORS - 2;

#[derive(Copy, Clone)]
pub enum Button {
    CallUp(Floor),
    CallDown(Floor),
    Internal(Floor),
}

#[derive(Copy, Clone)]
pub enum MotorDir {
    Up,
    Down,
    Stop,
}

#[derive(Copy, Clone)]
pub enum Light {
    On,
    Off,
}

#[derive(Copy, Clone)]
pub enum Signal {
    High,
    Low,
}

impl Signal {
    pub fn new(value: usize) -> Self {
        if value == 0 { Signal::Low }
        else          { Signal::High }
    }
}

const MOTOR_SPEED: usize = 2800;

impl ElevIo {
    fn initialize(&self) -> io::Result<()> {
        // Drive elevator to known floor
        self.set_motor_dir(MotorDir::Down)?;
        loop {
            match self.get_floor_signal()? {
                Floor::At(0) => break,
                _ => {},
            }
        }
        self.set_motor_dir(MotorDir::Stop)?;

        Ok(())
    }

    pub fn new() -> io::Result<Self> {
        let elev = ElevIo { io: HwIo::new()? };
        elev.set_all_light(Light::Off)?;
        elev.initialize()?;
        elev.set_floor_light(Floor::At(0))?;
        Ok(elev)
    }

    pub fn set_motor_dir(&self, dir: MotorDir) -> io::Result<()> {
        const MOTOR_ADDR: usize    = 0x100+0;
        const MOTORDIR_ADDR: usize = 0x300+15;
        match dir {
            MotorDir::Stop => self.io.write_analog(MOTOR_ADDR, 0)?,
            MotorDir::Up => {
                self.io.clear_bit(MOTORDIR_ADDR)?;
                self.io.write_analog(MOTOR_ADDR, MOTOR_SPEED)?;
            },
            MotorDir::Down => {
                self.io.set_bit(MOTORDIR_ADDR)?;
                self.io.write_analog(MOTOR_ADDR, MOTOR_SPEED)?;
            },
        };
        Ok(())
    }

    pub fn set_all_light(&self, mode: Light) -> io::Result<()> {
        for floor in 0..N_FLOORS {
            if floor != TOP { self.set_button_light(Button::CallUp(Floor::At(floor)), mode)?; }
            if floor != 0   { self.set_button_light(Button::CallDown(Floor::At(floor)), mode)?; }
            self.set_button_light(Button::Internal(Floor::At(floor)), mode)?;
        }
        self.set_stop_light(mode)?;
        self.set_door_light(mode)?;
        Ok(())
    }

    pub fn set_button_light(&self, button: Button, mode: Light) -> io::Result<()> {
        const CALL_UP_ADDR: [usize; 3]   = [ 0x300+9, 0x300+8, 0x300+6 ];
        const CALL_DOWN_ADDR: [usize; 3] = [ 0x300+7, 0x300+5, 0x300+4 ];
        const INTERNAL_ADDR: [usize; 4]  = [ 0x300+13, 0x300+12, 0x300+11, 0x300+10 ];
        let addr = match button {
            Button::CallUp(Floor::At(floor @ 0...SEC_TOP)) => CALL_UP_ADDR[floor],
            Button::CallDown(Floor::At(floor @ 1...TOP)) => CALL_DOWN_ADDR[floor-1],
            Button::Internal(Floor::At(floor @ 0...TOP)) => INTERNAL_ADDR[floor],
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "given floor is not supported for given button")),
        };
        match mode {
            Light::On => self.io.set_bit(addr)?,
            Light::Off => self.io.clear_bit(addr)?,
        };
        Ok(())

    }

    pub fn set_floor_light(&self, floor: Floor) -> io::Result<()> {
        const FLOOR_LIGHT_ADDR: [usize; 2] = [ 0x300+0, 0x300+1 ];
        if let Floor::At(etg) = floor {
            if etg > TOP {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "given floor is not supported"));
            }
            if etg & 0x2 != 0 { self.io.set_bit(FLOOR_LIGHT_ADDR[0])?; }
            else              { self.io.clear_bit(FLOOR_LIGHT_ADDR[0])?; }
            if etg & 0x1 != 0 { self.io.set_bit(FLOOR_LIGHT_ADDR[1])?; }
            else              { self.io.clear_bit(FLOOR_LIGHT_ADDR[1])?; }
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "Cannot set light between floors"))
        }
    }

    pub fn set_door_light(&self, mode: Light) -> io::Result<()> {
        const DOOR_LIGHT_ADDR: usize = 0x300+3;
        match mode {
            Light::On => self.io.set_bit(DOOR_LIGHT_ADDR)?,
            Light::Off => self.io.clear_bit(DOOR_LIGHT_ADDR)?,
        }
        Ok(())
    }

    pub fn set_stop_light(&self, mode: Light) -> io::Result<()> {
        const STOP_LIGT_ADDR: usize = 0x300+14;
        match mode {
            Light::On => self.io.set_bit(STOP_LIGT_ADDR)?,
            Light::Off => self.io.clear_bit(STOP_LIGT_ADDR)?,
        }
        Ok(())
    }

    pub fn get_button_signal(&self, button: Button) -> io::Result<Signal> {
        const CALL_UP_ADDR: [usize; 3] = [ 0x300+17, 0x300+16, 0x200+1 ];
        const CALL_DOWN_ADDR: [usize; 3] = [ 0x200+0, 0x200+2, 0x200+3 ];
        const INTERNAL_ADDR: [usize; 4] = [ 0x300+21, 0x300+20, 0x300+19, 0x300+18 ];
        let addr = match button {
            Button::CallUp(Floor::At(floor @ 0...SEC_TOP)) => CALL_UP_ADDR[floor],
            Button::CallDown(Floor::At(floor @ 1...TOP)) => CALL_DOWN_ADDR[floor-1],
            Button::Internal(Floor::At(floor @ 0...TOP)) => INTERNAL_ADDR[floor],
            _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "given floor is not supported for given button")),
        };
        let value = self.io.read_bit(addr)?;
        Ok(Signal::new(value))
    }

    pub fn get_floor_signal(&self) -> io::Result<Floor> {
        const FLOOR_SENSOR_ADDR: [usize; 4] = [ 0x200+4, 0x200+5, 0x200+6, 0x200+7 ];
        for (floor, addr) in FLOOR_SENSOR_ADDR.iter().enumerate() {
            if self.io.read_bit(*addr)? != 0 {
                return Ok(Floor::At(floor));
            }
        }
        Ok(Floor::Between)
    }

    pub fn get_stop_signal(&self) -> io::Result<Signal> {
        const STOP_SENSOR_ADDR: usize = 0x300+22;
        Ok(Signal::new(self.io.read_bit(STOP_SENSOR_ADDR)?))
    }

    pub fn get_obstr_signal(&self) -> io::Result<Signal> {
        const OBSTR_SENSOR_ADDR: usize = 0x300+23;
        Ok(Signal::new(self.io.read_bit(OBSTR_SENSOR_ADDR)?))
    }

}

#[cfg(test)]
mod tests {
    use super::ElevIo;

    #[test]
    fn test_elev_io_init() {
        assert!(ElevIo::new().is_ok(), "ElevIo::new failed");
    }
}
