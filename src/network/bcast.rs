
use std::io;
use std::net::UdpSocket;
use std::str::from_utf8;
use std::sync::mpsc;

extern crate serde;
extern crate serde_json;
extern crate net2;

pub struct BcastTransmitter {
    conn: UdpSocket,
}

impl BcastTransmitter {
    pub fn new(port: u16) -> io::Result<Self> {
        let conn = {
            let udp = try!(net2::UdpBuilder::new_v4());
            try!(udp.reuse_address(true));
            let socket = try!(udp.bind("0.0.0.0:0"));
            try!(socket.set_broadcast(true));
            try!(socket.connect(("10.22.71.255", port)));
            socket
        };
        Ok(BcastTransmitter {
            conn: conn,
        })
    }

    pub fn transmit<'a, T>(&self, data: &'a T) -> io::Result<()>
        where T: serde::ser::Serialize,
    {
        let serialized = serde_json::to_string(&data).unwrap();
        try!(self.conn.send(serialized.as_bytes()));
        Ok(())
    }

    pub fn run<T>(self, bcast_rx: mpsc::Receiver<T>) -> !
        where T: serde::ser::Serialize,
    {
        loop {
            let msg = bcast_rx.recv().unwrap();
            self.transmit(&msg).expect("Transmission of data failed for BcastTransmitter");
        }
    }
}

pub struct BcastReceiver {
    conn: UdpSocket,
}

impl BcastReceiver {
    pub fn new(port: u16) -> io::Result<Self> {
        let conn = {
            let udp = try!(net2::UdpBuilder::new_v4());
            try!(udp.reuse_address(true));
            let socket = try!(udp.bind(("10.22.71.255", port)));
            try!(socket.set_broadcast(true));
            socket
        };
        Ok(BcastReceiver {
            conn: conn,
        })
    }

    pub fn receive<T>(&self) -> io::Result<(T, String)>
        where T: serde::de::Deserialize,
    {
        let mut buf = [0u8; 1024];
        let (amt, addr) = try!(self.conn.recv_from(&mut buf));
        let msg = from_utf8(&buf[..amt]).unwrap();
        let json = serde_json::from_str(&msg).unwrap();
        Ok((json, addr.ip().to_string()))
    }

    pub fn run<T>(self, bcast_tx: mpsc::Sender<(T, String)>) -> !
        where T: serde::de::Deserialize,
    {
        loop {
            let (msg, addr) = match self.receive() {
                Ok(msg) => msg,
                Err(err) => {
                    println!("Recv failed for BcastReceiver. Error: {}", err);
                    continue;
                }
            };
            bcast_tx.send((msg, addr)).unwrap();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use std::net::IpAddr;

    use localip::get_localip;

    // Custom Type
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    enum Values {
        Hello,
        Integer(i32),
        Float(f32),
    }

    #[test]
    fn transmitter_works() {
        let port = 7000;
        let transmitter = BcastTransmitter::new(port).unwrap();
        let msg = "Test String".to_string();
        assert_eq!(transmitter.transmit(&msg).is_ok(), true);
    }

    #[test]
    fn transmit_localip_to_reciever() {
        let port = 8000;
        let num_transfers = 10;
        let localip = get_localip().unwrap();
        thread::spawn(move || {
            let transmitter = BcastTransmitter::new(port).unwrap();
            for _ in 0..num_transfers {
                thread::sleep(Duration::new(0, 1_000_000));
                transmitter.transmit(&localip).unwrap();
            }
        });
        let receiver = BcastReceiver::new(port).unwrap();
        for _ in 0..num_transfers {
            assert_eq!(receiver.receive::<IpAddr>().unwrap(), localip);
        }
    }

    #[test]
    fn transmit_customtype_to_receiver() {
        let port = 9999;
        let values = vec![Values::Hello, Values::Integer(4), Values::Float(-3.3)];
        {
            let values = values.clone();
            thread::spawn(move || {
                let transmitter = BcastTransmitter::new(port).unwrap();
                for value in &values {
                    thread::sleep(Duration::new(0, 1_000_000));
                    transmitter.transmit(value).unwrap();
                }
            });
        }
        let receiver = BcastReceiver::new(port).unwrap();
        for value in values {
            assert_eq!(receiver.receive::<Values>().unwrap(), value);
        }
    }

}
