use crate::device::DeviceConfig;
use crate::mapping::MapConfig;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Configuration loader for device and mapping configurations
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load device configuration from a JSON file
    pub fn load_device_config<P: AsRef<Path>>(path: P) -> Result<DeviceConfig> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read device config file: {:?}", path.as_ref()))?;

        let config: DeviceConfig =
            serde_json::from_str(&content).with_context(|| "Failed to parse device config JSON")?;

        Ok(config)
    }

    /// Load mapping configuration from a JSON file
    pub fn load_map_config<P: AsRef<Path>>(path: P) -> Result<MapConfig> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read map config file: {:?}", path.as_ref()))?;

        let config: MapConfig =
            serde_json::from_str(&content).with_context(|| "Failed to parse map config JSON")?;

        Ok(config)
    }
}
