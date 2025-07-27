use crate::device::{Command, DeviceConfig, OscArg, TempoDataType, TempoSpec};
use crate::mapping::{Destination, MapConfig};
use crate::session_manager::SessionManager;
use anyhow::Result;
use midi_types::MidiMessage;
use rosc::{OscMessage, OscPacket, OscType};
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// MIDI event processor that handles incoming MIDI events and routes commands
pub struct MidiProcessor {
    device_config: Arc<RwLock<DeviceConfig>>,
    map_config: Arc<RwLock<MapConfig>>,
    osc_socket: Option<UdpSocket>,
    session_manager: Option<SessionManager>,
    current_bpm: Arc<tokio::sync::RwLock<Option<f64>>>,
    // Cancellation token for tap tempo operations
    tap_tempo_cancel_tx: tokio::sync::watch::Sender<u64>,
    tap_tempo_cancel_rx: tokio::sync::watch::Receiver<u64>,
}

impl MidiProcessor {
    pub fn new(
        device_config: Arc<RwLock<DeviceConfig>>,
        map_config: Arc<RwLock<MapConfig>>,
    ) -> Result<Self> {
        // Create a UDP socket for OSC messages
        let osc_socket = UdpSocket::bind("0.0.0.0:0").ok();

        // Create cancellation channel for tap tempo operations
        let (tap_tempo_cancel_tx, tap_tempo_cancel_rx) = tokio::sync::watch::channel(0u64);

        Ok(Self {
            device_config,
            map_config,
            osc_socket,
            session_manager: None,
            current_bpm: Arc::new(tokio::sync::RwLock::new(None)),
            tap_tempo_cancel_tx,
            tap_tempo_cancel_rx,
        })
    }

    /// Set the session manager after construction
    pub fn set_session_manager(&mut self, session_manager: SessionManager) {
        self.session_manager = Some(session_manager);
    }

    /// Process an incoming MIDI message
    pub async fn process_midi_message(&self, message: MidiMessage) -> Result<()> {
        match message {
            MidiMessage::ProgramChange(msg_channel, program) => {
                self.handle_program_change(msg_channel.into(), program.into())
                    .await?;
            }
            _ => {
                debug!("Ignoring MIDI message: {:?}", message);
            }
        }
        Ok(())
    }

    /// Handle OSC tempo message
    pub async fn handle_osc_tempo(&self, bpm: f64) -> Result<()> {
        info!("Tempo updated via OSC: {:.1} BPM", bpm);

        // Update current BPM
        {
            let mut current_bpm = self.current_bpm.write().await;
            *current_bpm = Some(bpm);
        }

        // Update tempo on all devices that support it
        self.update_device_tempos(bpm).await?;
        Ok(())
    }

    /// Handle MIDI Program Change messages
    async fn handle_program_change(&self, midi_channel: u8, program: u8) -> Result<()> {
        info!(
            "Program change received: channel {}, program {}",
            midi_channel, program
        );

        let map_config = self.map_config.read().await;
        let device_config = self.device_config.read().await;

        // Find device mappings that match the input channel
        for mapping in &map_config.device_mappings {
            if mapping.listen_channel == midi_channel {
                if let Some(device) = device_config.get_device(&mapping.device_id) {
                    // Find the program in the device
                    if let Some(device_program) =
                        device.programs.iter().find(|p| p.number == program)
                    {
                        info!(
                            "Executing program '{}' on device '{}'",
                            device_program.name, device.name
                        );

                        // Execute all commands for this program
                        for command in &device_program.commands {
                            self.execute_command(
                                command,
                                &mapping.destination,
                                mapping.send_channel,
                            )
                            .await?;
                        }
                    } else {
                        warn!("Program {} not found on device '{}'", program, device.name);
                    }
                } else {
                    warn!("Device '{}' not found in configuration", mapping.device_id);
                }
            }
        }

        Ok(())
    }

    /// Update tempo on all devices that have tempo specifications
    async fn update_device_tempos(&self, bpm: f64) -> Result<()> {
        // Cancel any ongoing tap tempo operations first
        let cancel_signal = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        if self.tap_tempo_cancel_tx.send(cancel_signal).is_err() {
            warn!("Failed to send tap tempo cancellation signal");
        }

        // Collect tempo update tasks while holding locks briefly
        let tempo_updates = {
            let map_config = self.map_config.read().await;
            let device_config = self.device_config.read().await;

            let mut updates = Vec::new();
            for mapping in &map_config.device_mappings {
                if let Some(device) = device_config.get_device(&mapping.device_id) {
                    if let Some(ref tempo_spec) = device.tempo_spec {
                        info!(
                            "Updating tempo for device '{}' to {:.1} BPM",
                            device.name, bpm
                        );
                        updates.push((
                            tempo_spec.clone(),
                            mapping.destination.clone(),
                            mapping.send_channel,
                        ));
                    }
                }
            }
            updates
        }; // Locks are released here

        // Execute tempo updates without holding any locks
        // Use a different ID for the new operations
        let operation_id = cancel_signal + 1; // Use a different ID for new operations

        // Set the channel to the new operation ID so new operations know they should continue
        if self.tap_tempo_cancel_tx.send(operation_id).is_err() {
            warn!("Failed to send new operation ID");
        }

        for (tempo_spec, destination, channel) in tempo_updates {
            self.send_tempo_update(&tempo_spec, bpm, &destination, channel, operation_id)
                .await?;
        }

        Ok(())
    }

    /// Send tempo update to a device
    async fn send_tempo_update(
        &self,
        tempo_spec: &TempoSpec,
        bpm: f64,
        destination: &Destination,
        channel: Option<u8>,
        cancel_id: u64,
    ) -> Result<()> {
        match tempo_spec {
            TempoSpec::TapTempo { commands } => {
                self.send_tap_tempo(commands, bpm, destination, channel, cancel_id)
                    .await?;
            }
            TempoSpec::RawTempo {
                commands,
                data_type,
            } => {
                self.send_raw_tempo(commands, data_type, bpm, destination, channel)
                    .await?;
            }
        }
        Ok(())
    }

    /// Send tap tempo (4 quarter note taps using the specified commands)
    async fn send_tap_tempo(
        &self,
        commands: &[Command],
        bpm: f64,
        destination: &Destination,
        channel: Option<u8>,
        cancel_id: u64,
    ) -> Result<()> {
        // Calculate interval between taps (quarter note duration in milliseconds)
        let quarter_note_ms = (60.0 / bpm * 1000.0) as u64;

        info!(
            "Sending tap tempo: 4 taps with {}ms intervals using {} commands (cancel_id: {})",
            quarter_note_ms,
            commands.len(),
            cancel_id
        );

        // Send 4 taps, each a quarter note apart
        for i in 0..4 {
            // Check if we've been cancelled
            if *self.tap_tempo_cancel_rx.borrow() != cancel_id {
                info!("Tap tempo cancelled (cancel_id: {})", cancel_id);
                return Ok(());
            }

            // Execute all commands for this tap
            for command in commands {
                self.execute_command(command, destination, channel).await?;
            }

            // Wait for the next tap (except after the last one)
            if i < 3 {
                // Use a cancellable sleep
                let sleep_future = tokio::time::sleep(Duration::from_millis(quarter_note_ms));
                let mut cancel_rx = self.tap_tempo_cancel_rx.clone();

                tokio::select! {
                    _ = sleep_future => {
                        // Sleep completed normally
                    }
                    _ = cancel_rx.changed() => {
                        // We've been cancelled
                        if *cancel_rx.borrow() != cancel_id {
                            info!("Tap tempo cancelled during sleep (cancel_id: {})", cancel_id);
                            return Ok(());
                        }
                    }
                }
            }
        }

        info!("Tap tempo completed (cancel_id: {})", cancel_id);
        Ok(())
    }

    /// Send raw tempo value using the specified commands
    async fn send_raw_tempo(
        &self,
        commands: &[Command],
        data_type: &TempoDataType,
        bpm: f64,
        destination: &Destination,
        channel: Option<u8>,
    ) -> Result<()> {
        // Calculate the value to send based on data type
        let value = match data_type {
            TempoDataType::Tempo => bpm, // Send BPM directly
            TempoDataType::Time => {
                // Send quarter note duration in milliseconds
                60.0 / bpm * 1000.0
            }
        };

        info!(
            "Sending raw tempo: {} = {:.1} (BPM: {:.1})",
            match data_type {
                TempoDataType::Tempo => "BPM",
                TempoDataType::Time => "quarter note ms",
            },
            value,
            bpm
        );

        // Execute all specified commands with the calculated value
        for command in commands {
            match command {
                Command::Osc { address, args } => {
                    // Replace any tempo placeholders in OSC arguments
                    let modified_args: Vec<OscArg> = args
                        .iter()
                        .map(|arg| match arg {
                            OscArg::Float { value: _ } => OscArg::Float {
                                value: value as f32,
                            },
                            OscArg::Int { value: _ } => OscArg::Int {
                                value: value as i32,
                            },
                            OscArg::Normalized { value: _, min, max } => OscArg::Float {
                                value: ((value as f32 - min) / (max - min)),
                            },
                            other => other.clone(),
                        })
                        .collect();

                    let osc_cmd = Command::Osc {
                        address: address.clone(),
                        args: modified_args,
                    };
                    self.execute_command(&osc_cmd, destination, channel).await?;
                }
                Command::ControlChange {
                    controller,
                    value: _,
                } => {
                    // Map value to MIDI CC range (0-127)
                    let cc_value = if matches!(data_type, TempoDataType::Tempo) {
                        // BPM range: assume 60-180 BPM maps to 0-127
                        ((value - 60.0) / 120.0 * 127.0).clamp(0.0, 127.0) as u8
                    } else {
                        // Time range: assume 333ms-1000ms (180-60 BPM) maps to 0-127
                        ((1000.0 - value) / 667.0 * 127.0).clamp(0.0, 127.0) as u8
                    };

                    let cc_cmd = Command::ControlChange {
                        controller: *controller,
                        value: cc_value,
                    };
                    self.execute_command(&cc_cmd, destination, channel).await?;
                }
                _ => {
                    // Execute other commands as-is
                    self.execute_command(command, destination, channel).await?;
                }
            }
        }

        Ok(())
    }

    /// Execute a command to the specified destination
    async fn execute_command(
        &self,
        command: &Command,
        destination: &Destination,
        channel: Option<u8>,
    ) -> Result<()> {
        match command {
            Command::ProgramChange { program } => {
                if let Some(ch) = channel {
                    self.send_midi_command(destination, ch, *program).await?;
                } else {
                    warn!("No channel specified for MIDI Program Change command");
                }
            }
            Command::ControlChange { controller, value } => {
                if let Some(ch) = channel {
                    self.send_midi_control_change(destination, ch, *controller, *value)
                        .await?;
                } else {
                    warn!("No channel specified for MIDI Control Change command");
                }
            }
            Command::Osc { address, args } => {
                self.send_osc_command(destination, address, args).await?;
            }
        }
        Ok(())
    }

    /// Send MIDI Program Change command
    async fn send_midi_command(
        &self,
        destination: &Destination,
        channel: u8,
        program: u8,
    ) -> Result<()> {
        match destination {
            Destination::RtpMidi { session_name } => {
                info!(
                    "Sending MIDI Program Change to session '{}': channel {}, program {}",
                    session_name, channel, program
                );

                if let Some(ref session_manager) = self.session_manager {
                    use midi_types::{Channel, Program};
                    let midi_channel = Channel::new(channel.saturating_sub(1) & 0x0F);
                    let midi_program = Program::new(program & 0x7F);
                    let message = MidiMessage::ProgramChange(midi_channel, midi_program);

                    session_manager
                        .send_midi_to_session(session_name, message)
                        .await?;
                } else {
                    warn!(
                        "No session manager available for session '{}'",
                        session_name
                    );
                }
            }
            Destination::Osc { destination_name } => {
                // Look up the OSC destination by name
                let map_config = self.map_config.read().await;
                if let Some(osc_dest) = map_config.osc_destinations.get(destination_name) {
                    warn!(
                        "Cannot send MIDI command to OSC destination '{}' ({}:{})",
                        destination_name, osc_dest.host, osc_dest.port
                    );
                } else {
                    warn!(
                        "OSC destination '{}' not found in configuration",
                        destination_name
                    );
                }
            }
        }
        Ok(())
    }

    /// Send MIDI Control Change command
    async fn send_midi_control_change(
        &self,
        destination: &Destination,
        channel: u8,
        controller: u8,
        value: u8,
    ) -> Result<()> {
        match destination {
            Destination::RtpMidi { session_name } => {
                info!(
                    "Sending MIDI Control Change to session '{}': channel {}, controller {}, value {}",
                    session_name, channel, controller, value
                );

                if let Some(ref session_manager) = self.session_manager {
                    use midi_types::{Channel, Control, Value7};
                    let midi_channel = Channel::new(channel.saturating_sub(1) & 0x0F);
                    let midi_controller = Control::new(controller & 0x7F);
                    let midi_value = Value7::new(value & 0x7F);
                    let message =
                        MidiMessage::ControlChange(midi_channel, midi_controller, midi_value);

                    session_manager
                        .send_midi_to_session(session_name, message)
                        .await?;
                } else {
                    warn!(
                        "No session manager available for session '{}'",
                        session_name
                    );
                }
            }
            Destination::Osc { destination_name } => {
                // Look up the OSC destination by name
                let map_config = self.map_config.read().await;
                if let Some(osc_dest) = map_config.osc_destinations.get(destination_name) {
                    warn!(
                        "Cannot send MIDI command to OSC destination '{}' ({}:{})",
                        destination_name, osc_dest.host, osc_dest.port
                    );
                } else {
                    warn!(
                        "OSC destination '{}' not found in configuration",
                        destination_name
                    );
                }
            }
        }
        Ok(())
    }

    /// Send OSC command
    async fn send_osc_command(
        &self,
        destination: &Destination,
        address: &str,
        args: &[OscArg],
    ) -> Result<()> {
        match destination {
            Destination::Osc { destination_name } => {
                // Look up the OSC destination by name
                let map_config = self.map_config.read().await;
                if let Some(osc_dest) = map_config.osc_destinations.get(destination_name) {
                    if let Some(ref socket) = self.osc_socket {
                        let osc_args: Vec<OscType> = args
                            .iter()
                            .map(|arg| match arg {
                                OscArg::Int { value } => OscType::Int(*value),
                                OscArg::Float { value } => OscType::Float(*value),
                                OscArg::String { value } => OscType::String(value.clone()),
                                OscArg::Bool { value } => OscType::Bool(*value),
                                OscArg::Normalized { value, min, max } => {
                                    OscType::Float((value - min) / (max - min))
                                }
                            })
                            .collect();

                        let msg = OscMessage {
                            addr: address.to_string(),
                            args: osc_args,
                        };

                        let packet = OscPacket::Message(msg);
                        let msg_buf = rosc::encoder::encode(&packet)?;

                        let addr = format!("{}:{}", osc_dest.host, osc_dest.port);
                        socket.send_to(&msg_buf, &addr)?;

                        info!(
                            "Sent OSC message to {} ({}): {} {:?}",
                            destination_name, addr, address, args
                        );
                    } else {
                        error!("OSC socket not available");
                    }
                } else {
                    warn!(
                        "OSC destination '{}' not found in configuration",
                        destination_name
                    );
                }
            }
            Destination::RtpMidi { session_name } => {
                warn!(
                    "Cannot send OSC command to RTP MIDI session '{}'",
                    session_name
                );
            }
        }
        Ok(())
    }
}
