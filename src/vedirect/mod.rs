use std::str::FromStr;
use std::vec::Vec;

#[derive(Debug, Copy, Clone)]
pub enum Register {
    MainVoltage(f32),
    AuxVoltage(f32),
    PanelVoltage(f32),
    PanelPower(f32),
    MainCurrent(f32),
    LoadCurrent(f32),
    InstantaneousPower(f32),
    ConsumedAmpHours(f32),
    StateOfCharge(f32),
    ACOutputVoltage(f32),
    ACOutputApparentPower(f32),
    TrackerOperationMode(),
    DcMonitorMode(),
}

#[derive(Debug, Copy, Clone)]
pub struct ParseRegisterError;

impl std::str::FromStr for Register {
    type Err = ParseRegisterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let r: Vec<&str> = s.split('\t').collect();

        if r.len() != 2 {
            return Err(ParseRegisterError);
        }

        match r[0] {
            "V" => {
                let v = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::MainVoltage(v / 1000f32))
            },
            "VS" => {
                let v = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::AuxVoltage(v / 1000f32))
            },
            "VPV" => {
                let v = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::PanelVoltage(v / 1000f32))
            },
            "PPV" => {
                let p = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::PanelPower(p))
            },
            "I" => {
                let i = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::MainCurrent(i / 1000f32))
            },
            "IL" => {
                let i = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::LoadCurrent(i / 1000f32))
            },
            "P" => {
                let p = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::InstantaneousPower(p / 1000f32))
            },
            "CE" => {
                let ah = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::ConsumedAmpHours(ah / 1000f32))
            },
            "SOC" => {
                let soc = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::StateOfCharge(soc / 10f32))
            },
            "AC_OUT_V" => {
                let v = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::ACOutputVoltage(v / 100f32))
            },
            "AC_OUT_S" => {
                let p = r[1].parse::<f32>().map_err(|_| ParseRegisterError)?;
                Ok(Register::ACOutputApparentPower(p))
            },
            "MPPT" => Ok(Register::TrackerOperationMode()),
            "MON" => Ok(Register::DcMonitorMode()),
            _ => Err(ParseRegisterError)
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum VeDirectDevice {
    BMV71xSmartShunt, // TODO: support BMVs independently
    MPPT,
    PhoenixInverter,
}

enum ParseState {
    Idle,
    SohReceived,
    HeaderReceived,
}

pub struct VeDirectParser {
    buf: Vec<u8>,
    device: Option<VeDirectDevice>,
    state: ParseState,
}

impl VeDirectParser {
    pub fn new() -> Self {
        VeDirectParser {
            buf: Vec::new(),
            device: None,
            state: ParseState::Idle,
        }
    }

    pub fn device(&self) -> Option<VeDirectDevice> {
        self.device
    }

    pub fn push(&mut self, buf: Option<&[u8]>) -> Option<Vec<Register>> {
        let mut regs: Vec<Register> = Vec::new();

        if buf.is_none() {
            if matches!(self.state, ParseState::HeaderReceived) {
                if let Ok(s) = std::str::from_utf8(&self.buf) {
                    if let Ok(r) = Register::from_str(s) {
                        regs.push(r.clone());
                        self.predict_device(&r);
                    }
                }
            }

            self.reset();
        }

        for b in buf.unwrap() {
            match self.state {
                ParseState::Idle => {
                    if *b == 0x0d {
                        self.state = ParseState::SohReceived;
                        continue;
                    }
                },
                ParseState::SohReceived => {
                    if *b == 0x0a {
                        self.state = ParseState::HeaderReceived;
                        continue;
                    } else {
                        self.state = ParseState::Idle;
                    }
                },
                ParseState::HeaderReceived => {
                    if *b == 0x0d {
                        // we've received a new SOH, lets parse and reset
                        if let Ok(s) = std::str::from_utf8(&self.buf) {
                            if let Ok(r) = Register::from_str(s) {
                                regs.push(r.clone());
                                self.predict_device(&r);
                            }
                        }
                        self.reset();
                    } else {
                        self.buf.push(*b);
                    }
                },
            }
        }

        if regs.len() > 0 {
            Some(regs)
        } else {
            None
        }
    }

    fn predict_device(&mut self, r: &Register) {
        match r {
            Register::ACOutputVoltage(_) => {
                self.device = Some(VeDirectDevice::PhoenixInverter);
            },
            Register::StateOfCharge(_) => {
                self.device = Some(VeDirectDevice::BMV71xSmartShunt);
            },
            Register::TrackerOperationMode() => {
                self.device = Some(VeDirectDevice::MPPT);
            },
            _ => (),
        }
    }

    fn reset(&mut self) {
        self.state = ParseState::Idle;
        self.buf.clear();
    }
}
