use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RTP MIDI session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpMidiSession {
    /// Session name
    pub name: String,
    /// Port to listen on
    pub port: u16,
    /// Whether this session should be created as a listener
    pub listen: bool,
    /// Remote sessions to connect to (if any)
    pub connect_to: Vec<RtpMidiRemote>,
}

/// Remote RTP MIDI session to connect to
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpMidiRemote {
    /// Remote host address
    pub host: String,
    /// Remote port
    pub port: u16,
    /// Remote session name
    pub name: String,
}

/// OSC destination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscDestination {
    /// Destination host
    pub host: String,
    /// Destination port
    pub port: u16,
}

/// OSC listening source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscSource {
    /// Name of this OSC source
    pub name: String,
    /// Port to listen on for incoming OSC messages
    pub port: u16,
}

/// Destination for commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Destination {
    /// Send to RTP MIDI session
    #[serde(rename = "rtp_midi")]
    RtpMidi { session_name: String },
    /// Send to OSC destination (by name reference)
    #[serde(rename = "osc")]
    Osc { destination_name: String },
}

/// Device mapping - associates a device with input channel and output destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMapping {
    /// Device ID to use
    pub device_id: String,
    /// MIDI channel to listen on (1-16)
    pub listen_channel: u8,
    /// MIDI channel to send commands on (1-16) for MIDI destinations
    pub send_channel: Option<u8>,
    /// Destination for commands from this device
    pub destination: Destination,
}

/// Complete mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfig {
    /// RTP MIDI sessions to create
    pub rtp_midi_sessions: Vec<RtpMidiSession>,
    /// OSC destinations (for reference)
    pub osc_destinations: HashMap<String, OscDestination>,
    /// OSC listening sources (for incoming tempo and other messages)
    pub osc_sources: Vec<OscSource>,
    /// Device mappings
    pub device_mappings: Vec<DeviceMapping>,
}
