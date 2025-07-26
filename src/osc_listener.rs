use crate::mapping::OscSource;
use crate::processor::MidiProcessor;
use anyhow::Result;
use rosc::{OscPacket, OscType, decoder};
use std::net::UdpSocket;
use std::sync::Arc;
use tokio::task;
use tracing::{debug, error, info, warn};

/// OSC listener that handles incoming OSC messages
pub struct OscListener {
    processor: Arc<MidiProcessor>,
}

impl OscListener {
    pub fn new(processor: Arc<MidiProcessor>) -> Self {
        Self { processor }
    }

    /// Start listening for OSC messages on the specified sources
    pub async fn start_listeners(&self, osc_sources: &[OscSource]) -> Result<()> {
        for source in osc_sources {
            self.start_listener(source).await?;
        }
        Ok(())
    }

    /// Start a single OSC listener
    async fn start_listener(&self, source: &OscSource) -> Result<()> {
        info!(
            "Starting OSC listener '{}' on port {}",
            source.name, source.port
        );

        let socket = UdpSocket::bind(format!("0.0.0.0:{}", source.port))?;
        socket.set_nonblocking(true)?;

        let processor = Arc::clone(&self.processor);
        let source_name = source.name.clone();

        task::spawn(async move {
            let mut buf = [0u8; 1024];
            let socket = tokio::net::UdpSocket::from_std(socket).expect("Failed to convert socket");

            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((size, _addr)) => {
                        if let Err(e) = Self::handle_osc_packet(&processor, &buf[..size]).await {
                            error!("Error handling OSC packet on '{}': {}", source_name, e);
                        }
                    }
                    Err(e) => {
                        error!("Error receiving OSC data on '{}': {}", source_name, e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle an incoming OSC packet
    async fn handle_osc_packet(processor: &Arc<MidiProcessor>, data: &[u8]) -> Result<()> {
        match decoder::decode_udp(data) {
            Ok((_, packet)) => {
                Self::process_osc_packet(processor, packet).await?;
            }
            Err(e) => {
                warn!("Failed to decode OSC packet: {}", e);
            }
        }
        Ok(())
    }

    /// Process a decoded OSC packet
    async fn process_osc_packet(processor: &Arc<MidiProcessor>, packet: OscPacket) -> Result<()> {
        match packet {
            OscPacket::Message(msg) => {
                debug!("Received OSC message: {} {:?}", msg.addr, msg.args);

                // Handle tempo messages
                if msg.addr == "/tempo/raw" {
                    if let Some(OscType::Float(bpm)) = msg.args.first() {
                        processor.handle_osc_tempo(*bpm as f64).await?;
                    } else if let Some(OscType::Int(bpm)) = msg.args.first() {
                        processor.handle_osc_tempo(*bpm as f64).await?;
                    } else {
                        warn!("Invalid argument type for /tempo/raw: {:?}", msg.args);
                    }
                }
                // Add more OSC message handlers here as needed
            }
            OscPacket::Bundle(bundle) => {
                // Handle OSC bundles by processing each packet
                for packet in bundle.content {
                    Box::pin(Self::process_osc_packet(processor, packet)).await?;
                }
            }
        }
        Ok(())
    }
}
