use rusb::UsbContext;
use serialport::SerialPort;
use slint::{Timer, TimerMode};
use std::path::Path;

use crate::usb::{UsbEvent, UsbHotPlugHandler};
use crate::vedirect::{VeDirectDevice, Register, VeDirectParser};

slint::include_modules!();

struct VeDirectPort {
    busnum: u8,
    portnum: u8,
    parser: VeDirectParser,
    port: Box<dyn SerialPort>,
    timer: Timer,
}

pub struct App {
    window: MainWindow,
    usb_reg: Option<rusb::Registration<rusb::Context>>,
}

impl App {
    pub fn new() -> Self {
        App {
            window: MainWindow::new().unwrap(),
            usb_reg: None,
        }
    }

    pub fn init(&mut self) {
        if !rusb::has_hotplug() {
            println!("hotplug not supported, hotplug support required");
            return;
        }

        match rusb::Context::new() {
            Ok(context) => self.initialize_usb(context),
            Err(_) => {
                println!("error initializing USB context");
                return;
            },
        }
    }

    pub fn run(&mut self) {
        self.window.run().unwrap();
    }

    fn initialize_usb(&mut self, context: rusb::Context) {
        let handler = UsbHotPlugHandler::new();
        let rx = handler.receiver();
        let weak_window = self.window.as_weak();
        let weak_window_copy = self.window.as_weak();

        let _ = weak_window.upgrade_in_event_loop(move |mw| {
            mw.set_inverter_power(798.9);
            mw.set_inverter_current(-126.238);
            mw.set_inverter_online(true);
        });

        let _ = Timer::single_shot(std::time::Duration::from_millis(2000), move || {
            weak_window_copy.upgrade_in_event_loop(move |mw| {
                mw.set_inverter_online(false);
            });
        });

        std::thread::spawn(move || {
            let mut ports: Vec<VeDirectPort> = Vec::new();

            loop {
                if let Ok(msg) = rx.try_recv() {
                    match msg {
                        UsbEvent::DeviceArrived{bus, port, config} => {
                            println!("found new USB device {}-{}:{}", bus, port, config);

                            // TODO: give it some time to initialize, lets find a better way to do
                            // this
                            std::thread::sleep(std::time::Duration::from_millis(500));

                            if let Some(port) = try_open_port(bus, port, config) {
                                ports.push(port);
                            }
                        },
                        UsbEvent::DeviceLeft{bus, port} => {
                            println!("USB device {}-{} left", bus, port);
                            for i in 0..ports.len() {
                                if ports[i].busnum == bus && ports[i].portnum == port {
                                    // TODO: update UI indicating disconnection or timeout
                                    ports.swap_remove(i);
                                    break;
                                }
                            }
                        },
                    }
                }

                // handle IO
                for p in &mut ports {
                    if let Ok(bytes) = p.port.bytes_to_read() {
                        let mut buf = vec![0; bytes as usize];
                        match p.port.read(&mut buf) {
                            Ok(_) => {
                                for b in buf {
                                    if let Some(regs) = p.parser.push_one(b) {
                                        if let Some(dev) = p.parser.device() {

                                            p.timer.start(
                                                TimerMode::Repeated,
                                                std::time::Duration::from_secs(5),
                                                move || {
                                                    let _ = weak_window.upgrade_in_event_loop(move |mw| {
                                                        handle_vedirect_timeout(&mw, dev);
                                                    });
                                                });


                                            let _ = weak_window.upgrade_in_event_loop(move |mw| {
                                                for r in regs {
                                                    dispatch_vedirect_message(&mw, r, dev);
                                                }
                                            });
                                        }
                                    }
                                }
                            },
                            Err(_) => {},
                        }
                    }
                }
            }
        });

        self.usb_reg = Some(
            rusb::HotplugBuilder::new()
            .vendor_id(0x403)
            .product_id(0x6015)
            .enumerate(true)
            .register(&context, Box::new(handler)).unwrap());

        std::thread::spawn(move || {
            loop {
                let _ = context.handle_events(None);
            }
        });
    }
}

fn handle_vedirect_timeout(mw: &MainWindow, dev: VeDirectDevice) {
    match dev {
        VeDirectDevice::SmartShunt => mw.set_shunt_online(false),
        VeDirectDevice::SmartSolarMppt => mw.set_solar_online(false),
        VeDirectDevice::PhoenixInverter => mw.set_inverter_online(false),
    }
}

fn dispatch_vedirect_message(mw: &MainWindow, reg: Register, dev: VeDirectDevice) {
    match dev {
        VeDirectDevice::SmartShunt => {
            mw.set_shunt_online(true);

            match reg {
                Register::AuxVoltage(v) => {
                    mw.set_truck_batt_voltage(v);
                },
                Register::StateOfCharge(soc) => {
                    mw.set_house_batt_level(soc / 100f32);
                },
                _ => (),
            }
        },
        VeDirectDevice::SmartSolarMppt => {
            mw.set_solar_online(true);

            match reg {
                Register::PanelPower(p) => {
                    mw.set_solar_power(p);
                }
                _ => (),
            }
        },
        VeDirectDevice::PhoenixInverter => {
            mw.set_inverter_online(true);

            match reg {
                Register::ACOutputApparentPower(p) => {
                    mw.set_inverter_power(p);
                },
                _ => (),
            }
        },
        _ => {},
    }
}

fn try_open_port(busnum: u8, portnum: u8, config: u8) -> Option<VeDirectPort> {
    if is_vedirect(busnum, portnum) {
        if let Some(tty) = resolve_tty(busnum, portnum, config) {
            if let Ok(port) = serialport::new(tty, 19_200)
                .timeout(std::time::Duration::from_millis(100))
                    .open() {
                        return Some(VeDirectPort {
                            busnum: busnum,
                            portnum: portnum,
                            parser: VeDirectParser::new(),
                            port: port,
                            timer: Timer::default(),
                        });
            }
        }
    }

    return None;
}

fn is_vedirect(bus: u8, port: u8) -> bool {
    if let Ok(devices) = rusb::DeviceList::new() {
        for d in devices.iter() {
            if d.bus_number() == bus && d.port_number() == port {
                if let (Ok(desc), Ok(handle)) = (d.device_descriptor(), d.open()) {
                    if let Ok(product) = handle.read_product_string_ascii(&desc) {
                        if product == "VE Direct cable" {
                            return true;
                        }
                    }
                }
            }
        }
    }

    return false;
}

fn resolve_tty(bus: u8, port: u8, config: u8) -> Option<String> {
    let base = format!("/sys/bus/usb/devices/usb{}/{}-{}/{}-{}:{}.{}",
        bus, bus, port, bus, port, config, 0);

    if let Ok(path) = std::fs::canonicalize(Path::new(base.as_str())) {
        for p in std::fs::read_dir(path).unwrap() {
            let filename = String::from(p.unwrap()
                .path().file_name().unwrap()
                .to_str().unwrap());

            if filename.contains("ttyUSB") {
                return Some(format!("/dev/{}", filename));
            }
        }
    }

    return None;
}
