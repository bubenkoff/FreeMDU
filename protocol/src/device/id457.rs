//! Device support for W 4xx series washing machines.
//!
//! Supports appliances with software ID 457, such as the W 471.
//! The firmware is closely related to software ID 469 (see [`id469`](crate::device::id469)),
//! but several memory addresses are shifted by one or two bytes.
//!
//! This implementation is read-only: it only unlocks read access, exposes no actions
//! and never writes to the appliance's memory. All addresses were determined by observing
//! a W 471 through the optical interface, without access to the ROM contents.
//!
//! A washing machine instance can be obtained using [`WashingMachine::connect`],
//! giving access to all device-specific methods the appliance offers.
//!
//! Alternatively, use [`device::connect`](crate::device::connect) to automatically detect
//! the device's software ID and return an appropriate device instance.

use crate::device::{
    Action, Date, Device, DeviceKind, Error, Interface, Property, PropertyKind, Result, Value,
    private, utils,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use bitflags_derive::{FlagsDebug, FlagsDisplay};
use core::{str, time::Duration};
use embedded_io_async::{Read, Write};
use strum::{Display, FromRepr};

macro_rules! compatible_software_ids {
    () => {
        457
    };
}
pub(super) use compatible_software_ids;

const PROP_MANUFACTURING_DATE: Property = Property {
    kind: PropertyKind::General,
    id: "manufacturing_date",
    name: "Manufacturing Date",
    unit: None,
};
const PROP_OPERATING_TIME: Property = Property {
    kind: PropertyKind::General,
    id: "operating_time",
    name: "Operating Time",
    unit: None,
};
const PROP_OPERATING_STATE: Property = Property {
    kind: PropertyKind::Operation,
    id: "operating_state",
    name: "Operating State",
    unit: None,
};
const PROP_SELECTED_PROGRAM: Property = Property {
    kind: PropertyKind::Operation,
    id: "selected_program",
    name: "Selected Program",
    unit: None,
};
const PROP_PROGRAM_TYPE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_type",
    name: "Program Type",
    unit: None,
};
const PROP_PROGRAM_TEMPERATURE: Property = Property {
    kind: PropertyKind::Operation,
    id: "program_temperature",
    name: "Program Temperature",
    unit: Some("°C"),
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
    unit: Some("rpm"),
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
const PROP_ACTIVE_ACTUATORS: Property = Property {
    kind: PropertyKind::Io,
    id: "active_actuators",
    name: "Active Actuators",
    unit: None,
};
const PROP_TEMPERATURE: Property = Property {
    kind: PropertyKind::Io,
    id: "temperature",
    name: "Temperature",
    unit: Some("°C"),
};
const PROP_WATER_LEVEL: Property = Property {
    kind: PropertyKind::Io,
    id: "water_level",
    name: "Water Level",
    unit: Some("mmH₂O"),
};

/// Washing machine operating state.
///
/// Different modes can be entered by pressing specific button combinations
/// when turning on the machine.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum OperatingState {
    /// The door is open. Washing programs cannot be started.
    DoorOpen,
    /// Default mode when the machine is turned on.
    ProgramIdle,
    /// A washing program is currently running.
    ProgramRunning,
    /// The washing program has finished.
    ProgramFinished,
    /// Service programming mode, providing access to all machine and program options.
    ///
    /// Entered by holding the _Delay start h/min_ and _Water plus_ buttons
    /// when turning on the machine.
    ServiceProgramming,
    /// Customer programming mode, with a limited subset of the service programming options.
    ///
    /// Entered by holding the _Intensive_/_Short_ and _Water plus_ buttons when turning on the machine.
    CustomerProgramming,
    /// Service mode. Allows viewing stored faults and testing actuators.
    ///
    /// Entered by holding the _Delay start h/min_ and _Intensive_/_Short_ buttons
    /// when turning on the machine.
    Service,
}

/// Washing machine program.
///
/// Each variant represents a position of the machine's program selector knob.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum Program {
    /// Finish position (no program selected).
    Finish,
    /// Cottons program, 95 °C.
    Cottons95,
    /// Cottons program, 75 °C.
    Cottons75,
    /// Cottons program, 60 °C.
    Cottons60,
    /// Cottons program, 40 °C.
    Cottons40,
    /// Cottons program, 30 °C.
    Cottons30,
    /// Minimum iron program, 60 °C.
    MinimumIron60,
    /// Minimum iron program, 50 °C.
    MinimumIron50,
    /// Minimum iron program, 40 °C.
    MinimumIron40,
    /// Minimum iron program, 30 °C.
    MinimumIron30,
    /// Drain/spin program.
    DrainSpin,
    /// Separate rinse program.
    SeparateRinse,
    /// Starch program.
    Starch,
    /// Mixed wash program, 40 °C.
    MixedWash40,
    /// Quick wash program, 40 °C.
    QuickWash40,
    /// Woolens program, cold.
    WoolensCold,
    /// Woolens program, 30 °C.
    Woolens30,
    /// Woolens program, 40 °C.
    Woolens40,
    /// Silks program, 30 °C.
    Silks30,
    /// Delicates program, cold.
    DelicatesCold,
    /// Delicates program, 30 °C.
    Delicates30,
    /// Delicates program, 40 °C.
    Delicates40,
}

/// Washing program type.
///
/// Represents the general category of a washing program.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramType {
    /// No program.
    None = 0x00,
    /// Cottons program.
    Cottons = 0x01,
    /// Minimum iron program.
    MinimumIron = 0x02,
    /// Delicates program.
    Delicates = 0x03,
    /// Woolens program.
    Woolens = 0x04,
    /// Quick wash program.
    QuickWash = 0x05,
    /// Starch program.
    Starch = 0x06,
    /// Drain/spin program.
    DrainSpin = 0x07,
    /// Separate rinse program.
    SeparateRinse = 0x09,
    /// Mixed wash program.
    MixedWash = 0x0a,
    /// Silks program.
    Silks = 0x0b,
}

bitflags::bitflags! {
    /// Washing program option.
    ///
    /// Each flag represents an optional feature that can be enabled for a program.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct ProgramOption: u8 {
        /// Soak option enabled.
        const Soak = 0x10;
        /// Pre-wash option enabled.
        const PreWash = 0x20;
        /// Water plus option enabled.
        const WaterPlus = 0x40;
        /// Intensive or short option enabled.
        ///
        /// The actual effect depends on the machine's programming configuration.
        const IntensiveShort = 0x80;
    }
}

/// Washing program spin setting.
///
/// The actual spin speed depends on the machine's programming configuration.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum SpinSetting {
    /// No spin.
    WithoutSpin,
    /// Rinse hold (spin is paused to prevent creasing).
    RinseHold,
    /// Minimum spin speed.
    SpinMin,
    /// Low spin speed.
    SpinLow,
    /// Medium spin speed.
    SpinMed,
    /// High spin speed.
    SpinHigh,
    /// Very high spin speed.
    SpinVeryHigh,
    /// Maximum spin speed.
    SpinMax,
}

/// Washing program phase.
///
/// Phases may not always execute in the defined order and some phases
/// may be skipped depending on the selected washing program.
#[derive(FromRepr, Display, PartialEq, Eq, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ProgramPhase {
    /// Program has not started yet.
    Idle,
    /// Program start scheduled with delay start function.
    DelayedStart,
    /// First soak/pre-wash phase.
    SoakPreWash1,
    /// Second soak/pre-wash phase.
    SoakPreWash2,
    /// Main wash phase.
    MainWash,
    /// First rinse phase.
    Rinse1,
    /// Second rinse phase.
    Rinse2,
    /// Third rinse phase.
    Rinse3,
    /// Fourth rinse phase.
    Rinse4,
    /// Fifth rinse phase.
    Rinse5,
    /// Rinse hold phase.
    RinseHold,
    /// Drain phase.
    Drain,
    /// Final spin phase.
    FinalSpin,
    /// Anti-crease/finish phase.
    AntiCreaseFinish,
}

bitflags::bitflags! {
    /// Washing machine actuator.
    ///
    /// Each flag represents a controllable component of the washing machine.
    /// Multiple actuators may be active simultaneously.
    #[derive(FlagsDisplay, FlagsDebug, PartialEq, Eq, Copy, Clone)]
    pub struct Actuator: u16 {
        /// Drain pump actuator.
        const DrainPump = 0x0004;
        /// Heater actuator.
        const Heater = 0x0020;
        /// Water diverter motor actuator.
        const WaterDiverter = 0x0080;
        /// Warm water valve actuator.
        const WarmWater = 0x0200;
        /// Cold water valve actuator.
        const ColdWater = 0x0400;
    }
}

/// Washing machine device implementation.
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
        // Read access is sufficient for all properties below.
        // Full access is deliberately not unlocked: the address of the ROM readout protection
        // flag is unknown for this software ID (0x02e9, as used by software ID 469,
        // is part of an EEPROM shadow area here and must not be written).
        intf.unlock_read_access(0x43ea).await?;

        Ok(Self { intf, software_id })
    }

    /// Queries the manufacturing/inspection date of the machine.
    pub async fn query_manufacturing_date(&mut self) -> Result<Date, P::Error> {
        let date: [u8; 4] = self.intf.read_eeprom(0x01ce).await?;

        Ok(Date::new(
            u16::from(date[0]) + u16::from(date[1]) * 100,
            date[2],
            date[3],
        ))
    }

    /// Queries the total operating time of the machine.
    ///
    /// The operating time is only incremented if a washing program is running.
    pub async fn query_operating_time(&mut self) -> Result<Duration, P::Error> {
        // The current time is stored as follows:
        //   - Minutes: binary value at 0x0052
        //   - Hours: BCD values from 0x0053 to 0x0055
        let time: [u8; 4] = self.intf.read_memory(0x0052).await?;
        let mins = time[0];
        let hours = utils::decode_bcd_value(u32::from_le_bytes([time[1], time[2], time[3], 0x00]));

        Ok(Duration::from_secs(
            (u64::from(hours) * 60 + u64::from(mins)) * 60,
        ))
    }

    /// Queries the operating state.
    pub async fn query_operating_state(&mut self) -> Result<OperatingState, P::Error> {
        OperatingState::from_repr(self.intf.read_memory(0x00cd).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the selected program.
    pub async fn query_selected_program(&mut self) -> Result<Program, P::Error> {
        Program::from_repr(self.intf.read_memory(0x00b5).await?).ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program type.
    ///
    /// The program type is set according to the program selector position.
    pub async fn query_program_type(&mut self) -> Result<ProgramType, P::Error> {
        ProgramType::from_repr(self.intf.read_memory(0x00dd).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program temperature.
    ///
    /// The program temperature is set according to the program
    /// selector position and provided in `°C` (degrees Celsius).
    /// Some programs use a slightly lower temperature than selected.
    pub async fn query_program_temperature(&mut self) -> Result<u8, P::Error> {
        Ok(self.intf.read_memory(0x00de).await?)
    }

    /// Queries the program options.
    ///
    /// The program options are set using the buttons on the front panel of the machine.
    pub async fn query_program_options(&mut self) -> Result<ProgramOption, P::Error> {
        ProgramOption::from_bits(self.intf.read_memory(0x0058).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program spin setting.
    pub async fn query_program_spin_setting(&mut self) -> Result<SpinSetting, P::Error> {
        SpinSetting::from_repr(self.intf.read_memory(0x0057).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the program spin speed.
    ///
    /// The spin speed is provided in `rpm` (revolutions per minute)
    /// and may not correspond exactly to the labels on the front panel.
    pub async fn query_program_spin_speed(&mut self) -> Result<u16, P::Error> {
        let speed: u8 = self.intf.read_memory(0x00e9).await?;

        Ok(u16::from(speed) * 50)
    }

    /// Queries the program phase.
    pub async fn query_program_phase(&mut self) -> Result<ProgramPhase, P::Error> {
        ProgramPhase::from_repr(self.intf.read_memory(0x00a2).await?)
            .ok_or(Error::UnexpectedMemoryValue)
    }

    /// Queries the contents of the seven-segment display.
    ///
    /// The machine typically displays the remaining time of the selected program.
    pub async fn query_display_contents(&mut self) -> Result<String, P::Error> {
        let display: [u8; 4] = self.intf.read_memory(0x009e).await?;

        Ok(utils::decode_mc14489_display(display)
            .iter()
            .flatten()
            .collect())
    }

    /// Queries the currently active actuators.
    pub async fn query_active_actuators(&mut self) -> Result<Actuator, P::Error> {
        let actuators: u16 = self.intf.read_memory(0x007d).await?;

        // Only the drain pump, heater and cold water valve bits have been observed so far.
        // Unknown bits are ignored instead of being treated as an error.
        Ok(Actuator::from_bits_truncate(actuators))
    }

    /// Queries the current temperature sensed by the NTC thermistor and the target temperature.
    ///
    /// The temperature is provided in `°C` (degrees Celsius).
    pub async fn query_temperature(&mut self) -> Result<(u8, u8), P::Error> {
        let [current, target] = self.intf.read_memory(0x0136).await?;

        Ok((current, target))
    }

    /// Queries the current water level sensed by the analog pressure sensor and the target level.
    ///
    /// The water level is provided in `mmH₂O` (millimeters of water).
    pub async fn query_water_level(&mut self) -> Result<(u8, u8), P::Error> {
        let [current, target] = self.intf.read_memory(0x007f).await?;

        Ok((current, target))
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

    fn software_id(&self) -> u16 {
        self.software_id
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::WashingMachine
    }

    fn properties(&self) -> &'static [Property] {
        &[
            PROP_MANUFACTURING_DATE,
            PROP_OPERATING_TIME,
            PROP_OPERATING_STATE,
            PROP_SELECTED_PROGRAM,
            PROP_PROGRAM_TYPE,
            PROP_PROGRAM_TEMPERATURE,
            PROP_PROGRAM_OPTIONS,
            PROP_PROGRAM_SPIN_SETTING,
            PROP_PROGRAM_SPIN_SPEED,
            PROP_PROGRAM_PHASE,
            PROP_DISPLAY_CONTENTS,
            PROP_ACTIVE_ACTUATORS,
            PROP_TEMPERATURE,
            PROP_WATER_LEVEL,
        ]
    }

    fn actions(&self) -> &'static [Action] {
        &[]
    }

    async fn query_property(&mut self, prop: &Property) -> Result<Value, P::Error> {
        match *prop {
            // General
            PROP_MANUFACTURING_DATE => Ok(self.query_manufacturing_date().await?.into()),
            PROP_OPERATING_TIME => Ok(self.query_operating_time().await?.into()),
            // Operation
            PROP_OPERATING_STATE => Ok(self.query_operating_state().await?.to_string().into()),
            PROP_SELECTED_PROGRAM => Ok(self.query_selected_program().await?.to_string().into()),
            PROP_PROGRAM_TYPE => Ok(self.query_program_type().await?.to_string().into()),
            PROP_PROGRAM_TEMPERATURE => Ok(self.query_program_temperature().await?.into()),
            PROP_PROGRAM_OPTIONS => Ok(self.query_program_options().await?.to_string().into()),
            PROP_PROGRAM_SPIN_SETTING => {
                Ok(self.query_program_spin_setting().await?.to_string().into())
            }
            PROP_PROGRAM_SPIN_SPEED => Ok(self.query_program_spin_speed().await?.into()),
            PROP_PROGRAM_PHASE => Ok(self.query_program_phase().await?.to_string().into()),
            PROP_DISPLAY_CONTENTS => Ok(self.query_display_contents().await?.into()),
            // Input/output
            PROP_ACTIVE_ACTUATORS => Ok(self.query_active_actuators().await?.to_string().into()),
            PROP_TEMPERATURE => Ok(self.query_temperature().await?.into()),
            PROP_WATER_LEVEL => Ok(self.query_water_level().await?.into()),
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
