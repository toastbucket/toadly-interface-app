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
    Pid(u16),
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
            "PID" => {
                let pid = u16::from_str_radix(&r[1][2..], 16).map_err(|_| ParseRegisterError)?;
                Ok(Register::Pid(pid))
            },
            _ => Err(ParseRegisterError)
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum VeDirectDevice {
    SmartShunt,
    SmartSolarMppt,
    PhoenixInverter,
}

#[derive(Debug)]
enum ParseState {
    Idle,
    RecordingLabel,
    RecordingValue,
    Checksum
}

const MAX_LABEL: usize = 8;
const MAX_VALUE: usize = 20;

pub struct VeDirectParser {
    device: Option<VeDirectDevice>,
    state: ParseState,
    regs: Vec<Register>,
    flabel: Vec<u8>,
    fvalue: Vec<u8>,
    checksum: u8,
}

impl VeDirectParser {
    pub fn new() -> Self {
        VeDirectParser {
            device: None,
            state: ParseState::Idle,
            regs: Vec::new(),
            flabel: Vec::new(),
            fvalue: Vec::new(),
            checksum: 0,
        }
    }

    pub fn device(&self) -> Option<VeDirectDevice> {
        self.device
    }

    pub fn push_one(&mut self, b: u8) -> Option<Vec<Register>> {
        let mut result: Option<Vec<Register>> = None;

        self.push_into_checksum(b);
        match self.state {
            ParseState::Idle => {
                if b == 0x0a {
                    self.state = ParseState::RecordingLabel;
                } else if b == 0x0d {
                    // valid, but nothing to do, skip
                } else {
                    // invalid value, broken message
                    self.reset();
                }
            },
            ParseState::RecordingLabel => {
                if b == 0x09 {
                    self.state = ParseState::RecordingValue;

                    if let Ok(s) = std::str::from_utf8(&self.flabel) {
                        if "Checksum" == s {
                            self.state = ParseState::Checksum;
                        }
                    }
                } else if self.flabel.len() > MAX_LABEL {
                    self.reset();
                } else {
                    self.flabel.push(b);
                }
            },
            ParseState::RecordingValue => {
                if b == 0x0d {
                    let rstring = format!("{}\t{}",
                                        std::str::from_utf8(&self.flabel).unwrap(),
                                        std::str::from_utf8(&self.fvalue).unwrap());
                    if let Ok(reg) = Register::from_str(rstring.as_ref()) {
                        if let Register::Pid(pid) = reg {
                            self.set_device(pid);
                        }
                        self.regs.push(reg);
                    }

                    self.flabel.clear();
                    self.fvalue.clear();
                    self.state = ParseState::Idle;
                } else if self.fvalue.len() > MAX_VALUE {
                    self.reset();
                } else {
                    self.fvalue.push(b);
                }
            },
            ParseState::Checksum => {
                if self.checksum == 0 {
                    result = Some(self.regs.clone());
                }
                self.reset();
            },
        }

        result
    }

    fn push_into_checksum(&mut self, byte: u8) {
        self.checksum = self.checksum.wrapping_add(byte);
    }

    fn set_device(&mut self, pid: u16) {
        self.device = match pid {
            0xa056 => Some(VeDirectDevice::SmartSolarMppt),
            0xa389..=0xa38b => Some(VeDirectDevice::SmartShunt),
            0xa2e9 => Some(VeDirectDevice::PhoenixInverter),
            _ => None,
        }
    }

    fn reset(&mut self) {
        self.state = ParseState::Idle;
        self.regs.clear();
        self.flabel.clear();
        self.fvalue.clear();
        self.checksum = 0;
    }
}
