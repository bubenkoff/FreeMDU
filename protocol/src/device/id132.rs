//! Device support for W 8xx/9xx series washing machines.
//!
//! Supports appliances with software ID 132, which typically use an EDPW 122 board or similar.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, Device, DeviceKind, Error, Fault, Interface, Property, PropertyKind, Result, Value,
    private, utils,
};
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
};
use bitflags_derive::{FlagsDebug, FlagsDisplay};
use core::{str, str::FromStr, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, EnumMessage, EnumString, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        132
    };
}
pub(super) use compatible_software_ids;

const PROP_OPERATING_TIME: Property = Property {
    kind: PropertyKind::General,
    id: "operating_time",
    name: "Operating Time",
    unit: None,
};
const PROP_FAULT_F1: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f1",
    name: "F1: Water Level Switch",
    unit: None,
};
const PROP_FAULT_F2: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f2",
    name: "F2: NTC Thermistor",
    unit: None,
};
const PROP_FAULT_F3: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f3",
    name: "F3: Heater",
    unit: None,
};
const PROP_FAULT_F4: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f4",
    name: "F4: Tachometer",
    unit: None,
};
const PROP_FAULT_F5: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f5",
    name: "F5: Detergent Overdose",
    unit: None,
};
const PROP_FAULT_F6: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f6",
    name: "F6: Water Inlet",
    unit: None,
};
const PROP_FAULT_F7: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f7",
    name: "F7: Drainage",
    unit: None,
};
const PROP_FAULT_F8: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f8",
    name: "F8: Final Spin Speed",
    unit: None,
};
const PROP_FAULT_F9: Property = Property {
    kind: PropertyKind::Fault,
    id: "fault_f9",
    name: "F9: EEPROM",
    unit: None,
};
const PROP_SELECTED_PROGRAM: Property = Property {
    kind: PropertyKind::Operation,
    id: "selected_program",
    name: "Selected Program",
    unit: None,
};
const PROP_PROGRAM_OPTIONS: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_options",
    name: "Program Options",
    unit: None,
};
const PROP_PROGRAM_SPIN_SETTING: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_spin_setting",
    name: "Program Spin Setting",
    unit: None,
};
const PROP_PROGRAM_SPIN_SPEED: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_spin_speed",
    name: "Program Spin Speed",
    unit: None,
};
const PROP_PROGRAM_PHASE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_phase",
    name: "Program Phase",
    unit: None,
};
const PROP_DISPLAY_CONTENTS: Property = Property {
    kind: PropertyKind::Operation,
    id: "display_contents",
    name: "Display Contents",
    unit: None,
};
const PROP_REMAINING_TIME: Property = Property {
    kind: PropertyKind::Operation,
    id: "remaining_time",
    name: "Remaining Time",
    unit: None,
};
const PROP_ACTIVE_ACTUATORS: Property = Property {
    kind: PropertyKind::Io,
    id: "active_actuators",
    name: "Active Actuators",
    unit: None,
};
const PROP_NTC_RESISTANCE: Property = Property {
    kind: PropertyKind::Io,
    id: "ntc_resistance",
    name: "NTC Resistance",
    unit: Some("Ω"),
};
const PROP_TARGET_TEMPERATURE: Property = Property {
    kind: PropertyKind::Io,
    id: "target_temperature",
    name: "Target Temperature",
    unit: Some("°C"),
};
const PROP_WATER_LEVEL: Property = Property {
    kind: PropertyKind::Io,
    id: "water_level",
    name: "Water Level",
    unit: None,
};
const PROP_TACHOMETER_SPEED: Property = Property {
    kind: PropertyKind::Io,
    id: "tachometer_speed",
    name: "Tachometer Speed",
    unit: Some("rpm"),
};
const PROP_SPINNING: Property = Property {
    kind: PropertyKind::Io,
    id: "spinning",
    name: "Spinning",
    unit: None,
};
const PROP_ESTIMATED_POWER: Property = Property {
    kind: PropertyKind::Io,
    id: "estimated_power",
    name: "Estimated Power",
    unit: Some("W"),
};

/// Washing machine fault code.
///
/// Each code represents a specific fault condition that can occur in the machine.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum FaultCode {
    /// Level switch (digital pressure sensor) fault.
    LevelSwitch,
    /// NTC thermistor (temperature sensor) fault.
    NtcThermistor,
    /// Heater fault.
    Heater,
    /// Tachometer generator fault.
    Tachometer,
    /// Detergent overdose fault.
    DetergentOverdose,
    /// Water inlet fault.
    WaterInlet,
    /// Drainage fault.
    Drainage,
    /// Final spin cycle speed too low (< 400 rpm) fault.
    FinalSpinSpeed,
    /// EEPROM fault.
    Eeprom,
}

/// Washing machine program.
///
/// Each variant represents a position of the machine's program selector knob.
#[derive(FromRepr, Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Program {
    /// Finish position (no program selected).
    #[strum(message = "Finish")]
    Finish,
    /// Cottons program, 95 °C.
    #[strum(message = "Cottons 95 °C")]
    Cottons95,
    /// Cottons program, 75 °C.
    #[strum(message = "Cottons 75 °C")]
    Cottons75,
    /// Cottons program, 60 °C.
    #[strum(message = "Cottons 60 °C")]
    Cottons60,
    /// Cottons program, 50 °C.
    #[strum(message = "Cottons 50 °C")]
    Cottons50,
    /// Cottons program, 40 °C.
    #[strum(message = "Cottons 40 °C")]
    Cottons40,
    /// Cottons program, 30 °C.
    #[strum(message = "Cottons 30 °C")]
    Cottons30,
    /// Minimum iron program, 60 °C.
    #[strum(message = "Minimum iron 60 °C")]
    MinimumIron60,
    /// Minimum iron program, 50 °C.
    #[strum(message = "Minimum iron 50 °C")]
    MinimumIron50,
    /// Minimum iron program, 40 °C.
    #[strum(message = "Minimum iron 40 °C")]
    MinimumIron40,
    /// Minimum iron program, 30 °C.
    #[strum(message = "Minimum iron 30 °C")]
    MinimumIron30,
    /// Delicates program, 60 °C.
    #[strum(message = "Delicates 60 °C")]
    Delicates60,
    /// Delicates program, 50 °C.
    #[strum(message = "Delicates 50 °C")]
    Delicates50,
    /// Delicates program, 40 °C.
    #[strum(message = "Delicates 40 °C")]
    Delicates40,
    /// Delicates program, 30 °C.
    #[strum(message = "Delicates 30 °C")]
    Delicates30,
    /// Delicates program, cold.
    #[strum(message = "Delicates cold")]
    DelicatesCold,
    /// Woolens program, 40 °C.
    #[strum(message = "Woolens 40 °C")]
    Woolens40,
    /// Woolens program, 30 °C.
    #[strum(message = "Woolens 30 °C")]
    Woolens30,
    /// Woolens program, cold.
    #[strum(message = "Woolens cold")]
    WoolensCold,
    /// Quick wash program, 40 °C.
    #[strum(message = "Quick wash 40 °C")]
    QuickWash40,
    /// Starch program.
    #[strum(message = "Starch")]
    Starch,
    /// Spin program.
    #[strum(message = "Spin")]
    Spin,
    /// Drain program.
    #[strum(message = "Drain")]
    Drain,
    /// Separate rinse program.
    #[strum(message = "Separate rinse")]
    SeparateRinse,
}

/// Washing program phase.
///
/// Phases may not always execute in the defined order and some phases
/// may be skipped depending on the selected washing program.
#[derive(FromRepr, Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramPhase {
    /// Program has not started yet.
    #[strum(message = "Not started")]
    Idle,
    /// Program start scheduled with delay start function.
    #[strum(message = "Delayed start")]
    DelayedStart,
    /// First soak/pre-wash phase.
    #[strum(message = "Soak/pre-wash 1")]
    SoakPreWash1,
    /// Second soak/pre-wash phase.
    #[strum(message = "Soak/pre-wash 2")]
    SoakPreWash2,
    /// Main wash phase.
    #[strum(message = "Main wash")]
    MainWash,
    /// First rinse phase.
    #[strum(message = "Rinse 1")]
    Rinse1,
    /// Second rinse phase.
    #[strum(message = "Rinse 2")]
    Rinse2,
    /// Third rinse phase.
    #[strum(message = "Rinse 3")]
    Rinse3,
    /// Fourth rinse phase.
    #[strum(message = "Rinse 4")]
    Rinse4,
    /// Fifth rinse phase.
    #[strum(message = "Rinse 5")]
    Rinse5,
    /// Rinse hold phase.
    #[strum(message = "Rinse hold")]
    RinseHold,
    /// Drain phase.
    #[strum(message = "Drain")]
    Drain,
    /// Final spin phase.
    #[strum(message = "Final spin")]
    FinalSpin,
    /// Anti-crease/finish phase.
    #[strum(message = "Anti-crease/finish")]
    AntiCreaseFinish,
}

bitflags::bitflags! {
    /// Washing program option.
    ///
    /// Each flag represents an option button on the front panel of the machine.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct ProgramOption: u8 {
        /// Water plus option enabled.
        const WaterPlus = 0x04;
        /// Pre-wash option enabled.
        const PreWash = 0x08;
        /// Soak option enabled.
        const Soak = 0x10;
        /// Intensive or short option enabled.
        ///
        /// The actual effect depends on the machine's programming configuration.
        const IntensiveShort = 0x20;
    }
}

/// Washing program spin setting.
///
/// Each variant represents a position of the machine's spin speed selector knob.
#[derive(Display, EnumString, EnumMessage, PartialEq, Eq, Copy, Clone, Debug)]
pub enum SpinSetting {
    /// Final spin at 1450 rpm.
    #[strum(message = "1450 rpm")]
    Spin1450,
    /// Final spin at 1200 rpm.
    #[strum(message = "1200 rpm")]
    Spin1200,
    /// Final spin at 900 rpm.
    #[strum(message = "900 rpm")]
    Spin900,
    /// Final spin at 600 rpm.
    #[strum(message = "600 rpm")]
    Spin600,
    /// Final spin at 400 rpm.
    #[strum(message = "400 rpm")]
    Spin400,
    /// Rinse hold (laundry is left in the water of the last rinse).
    #[strum(message = "Rinse hold")]
    RinseHold,
    /// No final spin.
    #[strum(message = "No final spin")]
    NoFinalSpin,
}

impl SpinSetting {
    /// Decodes the knob position from the spin setting and spin speed values.
    ///
    /// Neither value identifies the position alone, so the pairs observed on
    /// a W 925 are matched as a whole.
    fn from_raw(setting: u8, speed: u8) -> Option<Self> {
        match (setting, speed) {
            (0, 27) => Some(Self::Spin1450),
            (3, 25) => Some(Self::Spin1200),
            (6, 22) => Some(Self::Spin900),
            (5, 18) => Some(Self::Spin600),
            (15, 15) => Some(Self::Spin400),
            (9, 27) => Some(Self::RinseHold),
            (8, 27) => Some(Self::NoFinalSpin),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    /// Washing machine actuator.
    ///
    /// Each flag represents a controllable component of the washing machine.
    /// Multiple actuators may be active simultaneously.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u16 {
        /// Softener compartment actuator.
        const Softener = 0x0002;
        /// Pre-wash compartment actuator.
        const PreWash = 0x0004;
        /// Main wash compartment actuator.
        const MainWash = 0x0008;
        /// Drain pump actuator.
        const DrainPump = 0x0010;
        /// Warm water actuator.
        const WarmWater = 0x0020;
        /// Reverse relay actuator.
        const Reverse = 0x2000;
        /// Motor field switch relay actuator.
        const FieldSwitch = 0x4000;
        /// Heater actuator.
        const Heater = 0x8000;
    }
}

/// Power draw of a W 925, measured part by part in its service mode by its owner.
mod power {
    /// The heater, Miele 4748124, rated 2.5 kW at 230 V; measured with an energy meter.
    pub const HEATER: u32 = 2500;
    /// While a program runs: the motor tumbling drew 75 W and the drain pump 100 W.
    pub const RUNNING: u32 = 100;
    /// Added per 1000 rpm of drum speed: spins drew 250 W at 400 rpm to 330 W at 1450 rpm with the
    /// pump running, fitted to the top speed, where a final spin spends most of its time.
    pub const DRUM_PER_1000_RPM: u32 = 150;
    /// Switched on with no program running.
    pub const STANDBY: u32 = 8;
}

/// Drum speed above which the drum counts as spinning, in rpm. Anti-crease turns the drum at
/// 28 to 39 rpm every four minutes or so (target 30), and washing is slower than a spin.
const SPINNING_RPM: u16 = 100;

/// Segment patterns of the digits 0 to 9 on the display, as this firmware keeps them in RAM.
const DISPLAY_SEGMENTS: [u8; 10] = [0x7e, 0x28, 0x5b, 0x3b, 0x2d, 0x37, 0x77, 0x2a, 0x7f, 0x3f];

/// Decodes the three display digits (hours, tens of minutes, minutes) from their RAM bytes.
///
/// A digit holds its segment pattern, or for a moment its plain value 0 to 9 while the firmware
/// rewrites it; no pattern is below 10, so both read alike. A blank digit is 0x00, the same as a
/// plain 0, which is how a blank leading digit counts anyway. Returns `None` for any other byte.
fn decode_display(display: [u8; 3]) -> Option<[u8; 3]> {
    let mut digits = [0; 3];

    for (digit, byte) in digits.iter_mut().zip(display) {
        *digit = if byte <= 9 {
            byte
        } else {
            (0..).zip(DISPLAY_SEGMENTS).find(|&(_, seg)| seg == byte)?.0
        };
    }

    Some(digits)
}

/// Washing machine device implementation.
///
/// Connect to a compatible washing machine using [`WashingMachine::connect`].
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> freemdu::device::Result<(), freemdu::serial::PortError> {
/// use freemdu::device::{Device, id132::WashingMachine};
///
/// let mut port = freemdu::serial::open("/dev/ttyACM0")?;
/// let mut machine = WashingMachine::connect(&mut port).await?;
///
/// println!("Selected program: {}", machine.query_selected_program().await?);
/// println!("Program options: {}", machine.query_program_options().await?);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct WashingMachine<P> {
    intf: Interface<P>,
    software_id: u16,
}

impl<P: Read + Write> WashingMachine<P> {
    pub(crate) async fn initialize(
        mut intf: Interface<P>,
        software_id: u16,
    ) -> Result<Self, P::Error> {
        intf.unlock_read_access(0x15a8).await?;
        intf.unlock_full_access(0x703d).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the total operating time of the machine.
    ///
    /// The operating time is only incremented if a washing program is running.
    /// It is internally stored in minutes and hours but only the hours are displayed in the service mode.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        // The current time is stored as follows:
        //   - Minutes: binary value at 0x0012
        //   - Hours: BCD values from 0x0013 to 0x0015
        // When the minutes counter reaches 60, the hour value is incremented.
        let time: [u8; 4] = self.intf.read_memory(0x0012).await?;
        let mins = time[0];
        let hours = utils::decode_bcd_value(u32::from_le_bytes([time[1], time[2], time[3], 0x00]));

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the status of a fault identified by its fault code.
    ///
    /// Faults may be either currently active or stored persistently in EEPROM
    /// from a previous occurrence when the machine was powered off.
    /// Returned faults do not include operating hours or occurrence count information.
    pub async fn query_fault(&mut self, code: FaultCode) -> Result<Fault, P::Error> {
        let mut query =
            async |active: (u16, u8), stored: Option<(u16, u8)>| -> Result<Fault, P::Error> {
                let val: u8 = self.intf.read_memory(active.0.into()).await?;

                if (val & active.1) != 0x00 {
                    Ok(Fault::Active(None))
                } else if let Some(stored) = stored {
                    let val: u8 = self.intf.read_memory(stored.0.into()).await?;

                    if (val & stored.1) != 0x00 {
                        Ok(Fault::Stored(None))
                    } else {
                        Ok(Fault::Ok)
                    }
                } else {
                    Ok(Fault::Ok)
                }
            };

        match code {
            // Detergent overdose only has two states (ok/active)
            // That fault is stored but doesn't have a dedicated bit for the active state
            FaultCode::LevelSwitch => query((0x0061, 0x02), Some((0x000e, 0x01))),
            FaultCode::NtcThermistor => query((0x0061, 0x04), Some((0x000e, 0x02))),
            FaultCode::Heater => query((0x0004, 0x20), Some((0x000e, 0x04))),
            FaultCode::Tachometer => query((0x007a, 0x02), Some((0x000e, 0x08))),
            FaultCode::DetergentOverdose => query((0x000e, 0x10), None),
            FaultCode::WaterInlet => query((0x0004, 0x02), Some((0x000e, 0x20))),
            FaultCode::Drainage => query((0x0004, 0x04), Some((0x000e, 0x40))),
            FaultCode::FinalSpinSpeed => query((0x0037, 0x10), Some((0x000e, 0x80))),
            FaultCode::Eeprom => query((0x0131, 0x0c), Some((0x000f, 0x01))),
        }
        .await
    }

    /// Queries the selected program.
    pub async fn query_selected_program(&mut self) -> Result<Program, P::Error> {
        // The selected program is set from the value at 0x0124 after a short delay.
        // This value is also used to set the persistent program value at 0x0001.
        Program::from_repr(self.intf.read_memory(0x0114).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program options.
    ///
    /// The program options are set using the buttons on the front panel of the machine.
    pub async fn query_program_options(&mut self) -> Result<ProgramOption, P::Error> {
        // Bits that are not known options are kept, so that they show up in the output
        Ok(ProgramOption::from_bits_retain(
            self.intf.read_memory(0x006c).await?,
        ))
    }

    /// Queries the program spin setting.
    ///
    /// The setting is decoded from the values at 0x006d and 0x0059 together,
    /// the latter of which is also returned by [`Self::query_program_spin_speed`].
    pub async fn query_program_spin_setting(&mut self) -> Result<SpinSetting, P::Error> {
        let setting = self.intf.read_memory(0x006d).await?;
        let speed = self.query_program_spin_speed().await?;

        SpinSetting::from_raw(setting, speed).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program spin speed.
    pub async fn query_program_spin_speed(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x0059).await?)
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        // Program phases are defined in a lookup table at address 0xe753.
        // The phase is determined by reading the value at 0x0000 to index into this table,
        // keeping only the lower nibble of the resulting value.
        // This value is used to set the front panel indicator lights at 0x0038 and 0x0039.
        ProgramPhase::from_repr(self.intf.read_memory(0x001c).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the time on the display as hours, tens of minutes and minutes.
    async fn query_display(&mut self) -> Result<[u8; 3], P::Error> {
        // The display digits are kept at 0x001e to 0x0020 (see `decode_display`).
        decode_display(self.intf.read_memory(0x001e).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the contents of the display, as it shows them: `2 13`, `27` or `0`.
    ///
    /// The display shows the time left: until the start during a delayed start, then until the
    /// end of the program. Leading zeros stay dark, and there is no colon between the hour and the
    /// minutes. The firmware revises the time early in a program: a Cottons 60 on a W 925 started
    /// at 1:51 and showed 1:01 two minutes later, which then held to the end.
    pub async fn query_display_contents(&mut self) -> Result<String, P::Error> {
        let [hours, tens, mins] = self.query_display().await?;

        Ok(match (hours, tens) {
            (0, 0) => format!("{mins}"),
            (0, _) => format!("{tens}{mins}"),
            _ => format!("{hours} {tens}{mins}"),
        })
    }

    /// Queries the time left as shown on the display (see [`Self::query_display_contents`]).
    pub async fn query_remaining_time(&mut self) -> Result<Duration, P::Error> {
        let [hours, tens, mins] = self.query_display().await?;
        let total_mins = u64::from(hours) * 60 + u64::from(tens) * 10 + u64::from(mins);

        Ok(Duration::from_secs(total_mins * 60))
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        // The active actuators at 0x003a and 0x003b are
        // used to set the outputs at ports 5 and 6, respectively.
        Actuator::from_bits(self.intf.read_memory(0x003a).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the NTC thermistor resistance.
    ///
    /// The resistance in `Ω` (ohms) is calculated from the ADC voltage.
    pub async fn query_ntc_resistance(&mut self) -> Result<u32, P::Error> {
        let val: u8 = self.intf.read_memory(0x0021).await?;

        Ok(utils::ntc_resistance_from_adc(val))
    }

    /// Queries the target temperature.
    ///
    /// The temperature is provided in `°C` (degrees Celsius).
    pub async fn query_target_temperature(&mut self) -> Result<u8, P::Error> {
        // The ADC readings from the NTC thermistor are
        // not converted into a temperature value in °C.
        // For this reason, the readings are compared with static thresholds
        // that are defined in a lookup table at 0xadca.
        // The threshold is selected based on the target temperature index.
        const TEMPERATURES: [u8; 15] = [90, 21, 27, 32, 34, 37, 47, 57, 72, 77, 80, 82, 85, 86, 65];
        let target: u8 = self.intf.read_memory(0x005c).await?;

        TEMPERATURES
            .get(target as usize)
            .copied()
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the current water level sensed by the digital pressure sensor and the target level.
    ///
    /// The water level is provided as a discrete value and ranges from 0 to 3.
    pub async fn query_water_level(&mut self) -> Result<(u8, u8), P::Error> {
        // The current water level is determined by a subroutine at 0xbee0.
        // Target water levels are defined in a lookup table at address 0xe57d,
        // which is indexed by the value at 0x0000 and stored in 0x005b.
        // The lowest 3 bytes of this value are then used to set the target water level.
        let [current, target] = self.intf.read_memory(0x003c).await?;

        Ok((current, target))
    }

    /// Queries the current speed sensed by the tachometer generator and the target speed.
    ///
    /// The speed in `rpm` (revolutions per minute) is only provided
    /// by the machine during the spin phase.
    pub async fn query_tachometer_speed(&mut self) -> Result<(u16, u16), P::Error> {
        // The target speed is selected from a lookup table at 0xc71d,
        // based on the value of the memory at 0x006e multiplied by 2.
        // Motor control calculations are performed in a subroutine at 0xc747.
        let speed: [u8; 5] = self.intf.read_memory(0x006f).await?;
        let target = u32::from_le_bytes([speed[0], speed[1], 0x00, 0x00]);
        let current = u32::from_le_bytes([speed[2], speed[3], speed[4], 0x00]);

        Ok((
            utils::rpm_from_motor_speed(current),
            utils::rpm_from_motor_speed(target),
        ))
    }

    /// Queries whether the drum is spinning: from the start of a spin until it has nearly run down.
    ///
    /// Unlike software ID 457, this firmware keeps counting the drum speed while the drum runs
    /// down after a spin (880, 509, 211 rpm in one W 925 log), so the speed tells when the drum
    /// is still. Not yet confirmed as the moment the door can be opened.
    pub async fn query_spinning(&mut self) -> Result<bool, P::Error> {
        let (current, _) = self.query_tachometer_speed().await?;

        Ok(current > SPINNING_RPM)
    }

    /// Queries an estimate of the machine's power draw in `W` (watts), from what it is doing.
    ///
    /// The heater, a flat figure while a program runs or standby otherwise, and an amount that
    /// grows with the drum speed (see the constants). This firmware reports no operating state,
    /// so a program counts as running from a soak or pre-wash phase to the final spin.
    pub async fn query_estimated_power(&mut self) -> Result<u32, P::Error> {
        let running = !matches!(
            self.query_program_phase().await?,
            ProgramPhase::Idle | ProgramPhase::DelayedStart | ProgramPhase::AntiCreaseFinish
        );
        let heating = self
            .query_active_actuators()
            .await?
            .contains(Actuator::Heater);
        let (rpm, _) = self.query_tachometer_speed().await?;

        let base = if running {
            power::RUNNING
        } else {
            power::STANDBY
        };
        let heater = if heating { power::HEATER } else { 0 };

        Ok(base + heater + power::DRUM_PER_1000_RPM * u32::from(rpm) / 1000)
    }
}

#[async_trait::async_trait(?Send)]
impl<P: Read + Write> Device<P> for WashingMachine<P> {
    async fn connect(port: P) -> Result<Self, P::Error> {
        let mut intf = Interface::new(port);
        let id = intf.query_software_id().await?;

        match id {
            compatible_software_ids!() => Self::initialize(intf, id).await,
            _ => Err(Error::UnknownSoftwareId(id)),
        }
    }

    fn interface(&mut self) -> &mut Interface<P> {
        &mut self.intf
    }

    fn value_name(&self, prop: &Property, val: &Value) -> Option<&'static str> {
        let Value::String(code) = val else {
            return None;
        };

        match *prop {
            PROP_SELECTED_PROGRAM => Program::from_str(code).ok()?.get_message(),
            PROP_PROGRAM_SPIN_SETTING => SpinSetting::from_str(code).ok()?.get_message(),
            PROP_PROGRAM_PHASE => ProgramPhase::from_str(code).ok()?.get_message(),
            _ => None,
        }
    }

    fn software_id(&self) -> u16 {
        self.software_id
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::WashingMachine
    }

    fn properties(&self) -> &'static [Property] {
        &[
            PROP_OPERATING_TIME,
            PROP_FAULT_F1,
            PROP_FAULT_F2,
            PROP_FAULT_F3,
            PROP_FAULT_F4,
            PROP_FAULT_F5,
            PROP_FAULT_F6,
            PROP_FAULT_F7,
            PROP_FAULT_F8,
            PROP_FAULT_F9,
            PROP_SELECTED_PROGRAM,
            PROP_PROGRAM_OPTIONS,
            PROP_PROGRAM_SPIN_SETTING,
            PROP_PROGRAM_SPIN_SPEED,
            PROP_PROGRAM_PHASE,
            PROP_DISPLAY_CONTENTS,
            PROP_REMAINING_TIME,
            PROP_ACTIVE_ACTUATORS,
            PROP_NTC_RESISTANCE,
            PROP_TARGET_TEMPERATURE,
            PROP_WATER_LEVEL,
            PROP_TACHOMETER_SPEED,
            PROP_SPINNING,
            PROP_ESTIMATED_POWER,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_OPERATING_TIME => Ok(self.query_operating_time().await?.into()),
            // Fault
            PROP_FAULT_F1 => Ok(self.query_fault(FaultCode::LevelSwitch).await?.into()),
            PROP_FAULT_F2 => Ok(self.query_fault(FaultCode::NtcThermistor).await?.into()),
            PROP_FAULT_F3 => Ok(self.query_fault(FaultCode::Heater).await?.into()),
            PROP_FAULT_F4 => Ok(self.query_fault(FaultCode::Tachometer).await?.into()),
            PROP_FAULT_F5 => Ok(self.query_fault(FaultCode::DetergentOverdose).await?.into()),
            PROP_FAULT_F6 => Ok(self.query_fault(FaultCode::WaterInlet).await?.into()),
            PROP_FAULT_F7 => Ok(self.query_fault(FaultCode::Drainage).await?.into()),
            PROP_FAULT_F8 => Ok(self.query_fault(FaultCode::FinalSpinSpeed).await?.into()),
            PROP_FAULT_F9 => Ok(self.query_fault(FaultCode::Eeprom).await?.into()),
            // Operation
            PROP_SELECTED_PROGRAM => Ok(self.query_selected_program().await?.to_string().into()),
            PROP_PROGRAM_OPTIONS => Ok(self.query_program_options().await?.to_string().into()),
            PROP_PROGRAM_SPIN_SETTING => {
                Ok(self.query_program_spin_setting().await?.to_string().into())
            }
            PROP_PROGRAM_SPIN_SPEED => {
                Ok(self.query_program_spin_speed().await?.to_string().into())
            }
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_DISPLAY_CONTENTS => Ok(self.query_display_contents().await?.into()),
            PROP_REMAINING_TIME => Ok(self.query_remaining_time().await?.into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_NTC_RESISTANCE => Ok(self.query_ntc_resistance().await?.into()),
            PROP_TARGET_TEMPERATURE => Ok(self.query_target_temperature().await?.into()),
            PROP_WATER_LEVEL => Ok(self.query_water_level().await?.into()),
            PROP_TACHOMETER_SPEED => Ok(self.query_tachometer_speed().await?.into()),
            PROP_SPINNING => Ok(self.query_spinning().await?.into()),
            PROP_ESTIMATED_POWER => Ok(self.query_estimated_power().await?.into()),
            _ => Err(Error::UnknownProperty),
        }
    }

    async fn trigger_action(
        &mut self,
        _action: &Action,
        _param: Option<&str>,
    ) -> Result<(), P::Error> {
        Err(Error::UnknownAction)
    }
}

impl<P> private::Sealed for WashingMachine<P> {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{collections::vec_deque::VecDeque, string::ToString};

    #[test]
    fn decodes_spin_setting() {
        assert_eq!(SpinSetting::from_raw(0, 27), Some(SpinSetting::Spin1450));
        assert_eq!(SpinSetting::from_raw(15, 15), Some(SpinSetting::Spin400));
        assert_eq!(SpinSetting::from_raw(9, 27), Some(SpinSetting::RinseHold));
        assert_eq!(SpinSetting::from_raw(8, 27), Some(SpinSetting::NoFinalSpin));
        assert_eq!(SpinSetting::from_raw(0, 15), None);
    }

    #[test]
    fn decodes_display() {
        // 1:51, 27 with the tens caught as a plain value, 9, and blank
        assert_eq!(decode_display([0x28, 0x37, 0x28]), Some([1, 5, 1]));
        assert_eq!(decode_display([0x00, 0x02, 0x2a]), Some([0, 2, 7]));
        assert_eq!(decode_display([0x00, 0x00, 0x3f]), Some([0, 0, 9]));
        assert_eq!(decode_display([0x00, 0x00, 0x00]), Some([0, 0, 0]));
        assert_eq!(decode_display([0x10, 0x00, 0x7e]), None);
    }

    #[test]
    fn displays_program_options() {
        let opts = ProgramOption::from_bits_retain(0x18);
        assert_eq!(opts.to_string(), "PreWash | Soak");
        assert_eq!(ProgramOption::empty().to_string(), "");
    }

    #[test]
    fn value_names() {
        let dev = WashingMachine {
            intf: Interface::new(VecDeque::<u8>::new()),
            software_id: 132,
        };
        let name = |prop: &Property, code: &str| dev.value_name(prop, &code.to_string().into());

        assert_eq!(
            name(&PROP_SELECTED_PROGRAM, "MinimumIron40"),
            Some("Minimum iron 40 °C")
        );
        assert_eq!(
            name(&PROP_PROGRAM_SPIN_SETTING, "RinseHold"),
            Some("Rinse hold")
        );
        assert_eq!(name(&PROP_PROGRAM_PHASE, "FinalSpin"), Some("Final spin"));
        assert_eq!(name(&PROP_PROGRAM_PHASE, "NoSuchPhase"), None);
        assert_eq!(
            dev.value_name(&PROP_WATER_LEVEL, &Value::Sensor(1, 2)),
            None
        );
    }

    #[test]
    fn every_variant_has_a_name() {
        for repr in 0..=u8::MAX {
            let names = [
                Program::from_repr(repr).map(|v| v.get_message()),
                ProgramPhase::from_repr(repr).map(|v| v.get_message()),
            ];

            for name in names.into_iter().flatten() {
                assert!(name.is_some(), "value {repr:#04x} has no name");
            }
        }

        for setting in 0..=u8::MAX {
            for speed in 0..=u8::MAX {
                if let Some(spin) = SpinSetting::from_raw(setting, speed) {
                    assert!(spin.get_message().is_some(), "{spin} has no name");
                }
            }
        }
    }
}
