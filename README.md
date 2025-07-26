# MIDI Router

A Rust application for MIDI patch mapping that listens on RTP MIDI ports and routes MIDI Program Change and clock events to different destinations (RTP MIDI or OSC).

## Features

- **RTP MIDI Support**: Listen and send MIDI over network using RTP MIDI protocol
- **OSC Support**: Send OSC messages to audio equipment (e.g., Behringer X32)
- **Device Management**: Configure devices with programs and associated commands
- **Flexible Routing**: Map input channels to different output destinations
- **Program Change Handling**: Respond to MIDI Program Change events
- **MIDI Clock Support**: Handle MIDI timing clock events

## Architecture

### Device Configuration

Devices represent conceptual MIDI or OSC equipment with defined programs. Each device can be:
- **MIDI Device**: Sends MIDI commands (Program Change, Control Change)
- **OSC Device**: Sends OSC messages

Example device configuration (`config/devices.json`):

```json
{
  "devices": {
    "mooer_m2": {
      "id": "mooer_m2",
      "name": "Mooer M2",
      "device_type": "midi",
      "programs": [
        {
          "number": 0,
          "name": "Clean",
          "commands": [
            {
              "type": "program_change",
              "channel": 1,
              "program": 0
            }
          ]
        }
      ]
    }
  }
}
```

### Map Configuration

The map configuration specifies:
- RTP MIDI sessions to create and listen on
- Device mappings (which device listens on which channel)
- Routing destinations for each device

Example map configuration (`config/map.json`):

```json
{
  "rtp_midi_sessions": [
    {
      "name": "MainInput",
      "port": 5004,
      "listen": true,
      "connect_to": []
    }
  ],
  "device_mappings": [
    {
      "device_id": "mooer_m2",
      "listen_channel": 1,
      "destination": {
        "type": "rtp_midi",
        "session_name": "Output1"
      }
    }
  ]
}
```

## Configuration

### Device Types

- **midi**: Device that sends MIDI commands
- **osc**: Device that sends OSC commands

### Command Types

#### MIDI Commands
- `program_change`: Send MIDI Program Change
- `control_change`: Send MIDI Control Change

#### OSC Commands
- `osc`: Send OSC message with specified address and arguments

### Destination Types

- **rtp_midi**: Route to RTP MIDI session
- **osc**: Route to OSC destination (host:port)

## Usage

1. **Configure Devices**: Edit `config/devices.json` to define your MIDI/OSC devices and their programs
2. **Configure Mapping**: Edit `config/map.json` to set up RTP MIDI sessions and device routing
3. **Run the Application**:
   ```bash
   cargo run
   ```

## Example Workflow

1. MIDI controller sends Program Change message on channel 1
2. Application receives message on configured RTP MIDI session
3. Looks up device mapped to channel 1 (e.g., "mooer_m2")
4. Finds program definition for the received program number
5. Executes all commands defined for that program
6. Sends commands to the configured destination (RTP MIDI or OSC)

## Dependencies

- **rtpmidi**: RTP MIDI protocol support
- **rosc**: OSC (Open Sound Control) support
- **serde**: Configuration serialization
- **tokio**: Async runtime
- **tracing**: Logging

## Building

```bash
cargo build --release
```

## Development

The application is structured with these main modules:

- `device.rs`: Device and command definitions
- `mapping.rs`: RTP MIDI session and routing configuration
- `processor.rs`: MIDI event processing and command execution
- `router.rs`: RTP MIDI session management
- `config.rs`: Configuration loading and saving
- `main.rs`: Application entry point

## License

This project is licensed under the MIT License - see the LICENSE file for details.