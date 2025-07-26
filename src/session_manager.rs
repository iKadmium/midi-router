use anyhow::Result;
use midi_types::MidiMessage;
use rtpmidi::packets::midi_packets::rtp_midi_message::RtpMidiMessage;
use rtpmidi::sessions::rtp_midi_session::RtpMidiSession as AppleMidiSession;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Shared session manager that can be used by both router and processor
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<AppleMidiSession>>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_session(&self, name: String, session: Arc<AppleMidiSession>) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(name, session);
    }

    pub async fn send_midi_to_session(
        &self,
        session_name: &str,
        message: MidiMessage,
    ) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_name) {
            info!(
                "Sending MIDI message to session '{}': {:?}",
                session_name, message
            );

            let rtp_message = RtpMidiMessage::MidiMessage(message);
            session.send_midi(&rtp_message).await?;

            Ok(())
        } else {
            warn!("Session '{}' not found", session_name);
            Ok(())
        }
    }

    pub async fn get_session_names(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            sessions: Arc::clone(&self.sessions),
        }
    }
}
