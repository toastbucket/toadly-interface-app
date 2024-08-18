mod vedirect;

use rusb;
use rusb::UsbContext;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use vedirect::{Register, VeDirectDevice, VeDirectParser};

slint::include_modules!();

#[derive(Debug)]
enum SensorMessage {
    VeDirectMessage { r: Register, d: VeDirectDevice },
}

struct UsbHotPlugHandler {
    sender: mpsc::Sender<SensorMessage>,
}

impl<T: rusb::UsbContext> rusb::Hotplug<T> for UsbHotPlugHandler {
    fn device_arrived(&mut self, device: rusb::Device<T>) {
        println!("device arrived!");
        if let (Ok(desc), Ok(handle)) = (device.device_descriptor(), device.open()) {
            if let Ok(product) = handle.read_product_string_ascii(&desc) {
                if product != "VE Direct cable" {
                    return;
                }
            }
        } else {
            return;
        }

        let base = format!("/sys/bus/usb/devices/usb{}/{}-{}/{}-{}:{}.{}",
            device.bus_number(),
            device.bus_number(),
            device.port_number(),
            device.bus_number(),
            device.port_number(),
            device.active_config_descriptor().unwrap().number(),
            0);
        if let Ok(path) = std::fs::canonicalize(Path::new(base.as_str())) {
            for p in std::fs::read_dir(path).unwrap() {
                let filename = String::from(p.unwrap()
                        .path().file_name().unwrap()
                        .to_str().unwrap());

                if filename.contains("ttyUSB") {
                    let new_sender = self.sender.clone();
                    let _ = std::thread::spawn(move || {
                        vedirect_monitor(format!("/dev/{}", filename).as_ref(), new_sender);
                    });
                    return;
                }
            }
        } else {
            println!("couldn't find new device");
        }
    }

    fn device_left(&mut self, device: rusb::Device<T>) {
        println!("device left {:?}", device);
    }
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

fn vedirect_monitor(port: &str, sender: mpsc::Sender<SensorMessage>) {
    println!("launching new monitor on {}", port);
    let mut port = serialport::new(port, 19_200)
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
}

fn main() {
    let mw = MainWindow::new().unwrap();
    let mw_w = mw.as_weak();
    let (sender, receiver) = mpsc::channel::<SensorMessage>();

    if !rusb::has_hotplug() {
        println!("hotplug not supported, hotplug support required");
        return;
    }

    if let Ok(context) = rusb::Context::new() {
        let _ = std::thread::spawn(move || {
            let _reg: Option<rusb::Registration<rusb::Context>> = Some(
                rusb::HotplugBuilder::new()
                .vendor_id(0x403)
                .product_id(0x6015)
                .enumerate(true)
                .register(&context, Box::new(UsbHotPlugHandler { sender: sender })).unwrap());

            loop {
                context.handle_events(None);
            }
        });
    }

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

    mw.run().unwrap();
}
