use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of devices that can send commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    /// MIDI device that sends MIDI commands
    #[serde(rename = "midi")]
    Midi,
    /// OSC device that sends OSC commands
    #[serde(rename = "osc")]
    Osc,
}

/// A command that can be sent by a device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    /// MIDI Program Change command
    #[serde(rename = "program_change")]
    ProgramChange { channel: u8, program: u8 },
    /// MIDI Control Change command
    #[serde(rename = "control_change")]
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    /// OSC message command
    #[serde(rename = "osc")]
    Osc { address: String, args: Vec<OscArg> },
}

/// Type of argument for raw tempo commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TempoArgType {
    /// Send as OSC integer
    #[serde(rename = "osc_int")]
    OscInt,
    /// Send as OSC float
    #[serde(rename = "osc_float")]
    OscFloat,
    /// Send as MIDI Control Change
    #[serde(rename = "midi_cc")]
    MidiCC { channel: u8, controller: u8 },
    /// Send as OSC float
    #[serde(rename = "osc_normalized")]
    OscNormalized { min: f32, max: f32 },
}

/// OSC argument types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OscArg {
    #[serde(rename = "int")]
    Int { value: i32 },
    #[serde(rename = "float")]
    Float { value: f32 },
    #[serde(rename = "string")]
    String { value: String },
    #[serde(rename = "bool")]
    Bool { value: bool },
    #[serde(rename = "normalized")]
    Normalized { value: f32, min: f32, max: f32 },
}

/// A program definition for a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    /// Program number (0-127 for MIDI)
    pub number: u8,
    /// Human-readable name for the program
    pub name: String,
    /// Commands to execute when this program is activated
    pub commands: Vec<Command>,
}

/// Device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Unique identifier for the device
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Type of device (MIDI or OSC)
    pub device_type: DeviceType,
    /// Programs available on this device
    pub programs: Vec<Program>,
    /// Tempo update specification (optional)
    pub tempo_spec: Option<TempoSpec>,
}

/// Specification for how to update tempo on a device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TempoSpec {
    /// Send tap tempo (4 quarter note taps using specified commands)
    #[serde(rename = "tap_tempo")]
    TapTempo { commands: Vec<Command> },
    /// Send raw tempo value
    #[serde(rename = "raw_tempo")]
    RawTempo {
        commands: Vec<Command>,
        data_type: TempoDataType,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TempoDataType {
    /// Send tempo value (BPM)
    Tempo,
    /// Send quarter note time in milliseconds
    Time,
}

/// Collection of all device configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Map of device ID to device configuration
    pub devices: HashMap<String, Device>,
}

impl DeviceConfig {
    pub fn get_device(&self, id: &str) -> Option<&Device> {
        self.devices.get(id)
    }
}
