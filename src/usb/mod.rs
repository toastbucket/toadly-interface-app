use rusb;
use rusb::UsbContext;

#[derive(Debug)]
pub enum UsbEvent {
    DeviceArrived{ bus: u8, port: u8, config: u8 },
    DeviceLeft{ bus: u8, port: u8 },
}

pub struct UsbHotPlugHandler {
    event_tx: crossbeam::channel::Sender<UsbEvent>,
    event_rx: crossbeam::channel::Receiver<UsbEvent>,
}

impl UsbHotPlugHandler {
    pub fn new() -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();

        UsbHotPlugHandler {
            event_tx: tx,
            event_rx: rx
        }
    }

    pub fn receiver(&self) -> crossbeam::channel::Receiver<UsbEvent> {
        self.event_rx.clone()
    }
}

impl<T: UsbContext> rusb::Hotplug<T> for UsbHotPlugHandler {
    fn device_arrived(&mut self, device: rusb::Device<T>) {
        let _ = self.event_tx.send(UsbEvent::DeviceArrived {
            bus: device.bus_number(),
            port: device.port_number(),
            config: device.active_config_descriptor().unwrap().number(),
        });
    }

    fn device_left(&mut self, device: rusb::Device<T>) {
        let evt = UsbEvent::DeviceLeft {
            bus: device.bus_number(),
            port: device.port_number(),
        };
        let _ = self.event_tx.send(evt);
    }
}
