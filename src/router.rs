use crate::mapping::{MapConfig, RtpMidiSession};
use crate::processor::MidiProcessor;
use crate::session_manager::SessionManager;
use anyhow::Result;
use rand::RngCore;
use rtpmidi::sessions::events::event_handling::MidiMessageEvent;
use rtpmidi::sessions::invite_responder::InviteResponder;
use rtpmidi::sessions::rtp_midi_session::RtpMidiSession as AppleMidiSession;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use tracing::info;

/// Manages RTP MIDI sessions and routes messages to the processor
pub struct MidiRouter {
    session_manager: SessionManager,
    processor: Arc<MidiProcessor>,
}

impl MidiRouter {
    pub fn new(processor: Arc<MidiProcessor>, session_manager: SessionManager) -> Self {
        Self {
            session_manager,
            processor,
        }
    }

    /// Initialize RTP MIDI sessions based on configuration
    pub async fn initialize_sessions(&mut self, map_config: &MapConfig) -> Result<()> {
        for session_config in &map_config.rtp_midi_sessions {
            self.create_session(session_config).await?;
        }
        Ok(())
    }

    /// Create and start a single RTP MIDI session
    async fn create_session(&mut self, config: &RtpMidiSession) -> Result<()> {
        info!(
            "Creating RTP MIDI session '{}' on port {}",
            config.name, config.port
        );

        // Create the Apple MIDI session
        let ssrc: u32 = rand::rng().next_u32();
        let session =
            AppleMidiSession::start(config.port, &config.name, ssrc, InviteResponder::Accept)
                .await?;

        if config.listen {
            info!("Starting listener for session '{}'", config.name);
            let processor = Arc::clone(&self.processor);
            session
                .add_listener(MidiMessageEvent, move |(message, _timestamp)| {
                    // Spawn a task to handle incoming MIDI messages asynchronously
                    let processor = Arc::clone(&processor);
                    tokio::spawn(async move {
                        tracing::debug!("Received MIDI message in session {message:?}");
                        if let Err(e) = processor.process_midi_message(message).await {
                            tracing::error!("Error processing MIDI message: {}", e);
                        }
                    });
                })
                .await;
        }

        // Connect to remote sessions if specified
        for remote in &config.connect_to {
            info!(
                "Connecting session '{}' to {}:{} ({})",
                config.name, remote.host, remote.port, remote.name
            );

            // Resolve hostname to socket address
            let addr_str = format!("{}:{}", remote.host, remote.port);
            let addr = addr_str
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| anyhow::anyhow!("Failed to resolve address: {}", addr_str))?;

            session.invite_participant(addr).await;
        }

        self.session_manager
            .add_session(config.name.clone(), session)
            .await;
        Ok(())
    }

    /// Get list of active session names
    pub async fn get_session_names(&self) -> Vec<String> {
        self.session_manager.get_session_names().await
    }
}
