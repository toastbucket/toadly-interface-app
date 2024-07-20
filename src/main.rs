mod vedirect;

use std::io::ErrorKind;
use std::sync::mpsc;
use std::time::Duration;
use vedirect::{Register, VeDirectDevice, VeDirectParser};

slint::include_modules!();

#[derive(Debug)]
enum SensorMessage {
    VeDirectMessage { r: Register, d: VeDirectDevice },
}

fn dispatch_vedirect_message(mw: &MainWindow, r: Register, d: VeDirectDevice) {
    match d {
        VeDirectDevice::BMV71xSmartShunt => {
            match r {
                Register::AuxVoltage(v) => {
                    mw.set_truck_batt_voltage(v);
                },
                Register::StateOfCharge(soc) => {
                    mw.set_house_batt_level(soc / 100f32);
                },
                _ => (),
            }
        },
        VeDirectDevice::MPPT => {
            match r {
                Register::PanelPower(p) => {
                    mw.set_solar_value(p);
                }
                _ => (),
            }
        },
        VeDirectDevice::PhoenixInverter => {
            match r {
                Register::ACOutputApparentPower(p) => {
                    mw.set_inverter_value(p);
                },
                _ => (),
            }
        },
        _ => println!("invalid VeDirectDevice: {:?}", d),
    }
}

fn main() {
    let mw = MainWindow::new().unwrap();
    let mw_w = mw.as_weak();
    let (sender, receiver) = mpsc::channel::<SensorMessage>();

    let _ = std::thread::spawn(move || {
        loop {
            if let Ok(msg) = receiver.recv() {
                let _ = mw_w.upgrade_in_event_loop(move |mw| {
                    match msg {
                        SensorMessage::VeDirectMessage { r, d } => {
                            dispatch_vedirect_message(&mw, r, d);
                        },
                        _ => (),
                    }
                });
            }
        }
    });

    let _ = std::thread::spawn(move || {
        let mut port = serialport::new("/tmp/vmodem0", 19_200)
            .timeout(Duration::from_millis(250))
            .open().expect("failed to open port");
        let mut veparser = VeDirectParser::new();

        loop {
            let mut buf = [0; 128];
            match port.read(&mut buf) {
                Ok(n) => {
                    let to_push: Option<&[u8]> = if n > 0 {Some(&buf)} else {None};
                    if let (Some(regs), Some(dev)) = (veparser.push(to_push), veparser.device()) {
                        for r in regs {
                            let _ = sender.send(SensorMessage::VeDirectMessage { r: r, d: dev });
                        }
                    }
                },
                Err(e) => {
                    if e.kind() == ErrorKind::TimedOut {
                        continue;
                    } else {
                        println!("{}", e);
                        break;
                    }
                },
            }
        }
    });

    mw.run().unwrap();
}
