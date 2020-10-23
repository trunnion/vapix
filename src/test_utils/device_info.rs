use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub model: String,
    pub serial_number: String,
    pub firmware_version: String,
    pub firmware_build_date: Option<String>,
    pub architecture: Option<String>,
    pub soc: Option<String>,
    pub hardware_id: Option<String>,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            model: "".to_string(),
            serial_number: "".to_string(),
            firmware_version: "".to_string(),
            firmware_build_date: None,
            architecture: None,
            soc: None,
            hardware_id: None,
        }
    }
}

impl DeviceInfo {
    pub async fn retrieve<T: Transport>(device: &Device<T>) -> Result<Self, Error> {
        let definitions = device
            .parameters()
            .list_definitions(Some(
                &["root.Properties.Firmware", "root.Properties.System"][..],
            ))
            .await?;

        let crate::v3::parameters::ParameterDefinitions {
            model,
            firmware_version,
            groups,
            ..
        } = definitions;

        let mut metadata = Self {
            model: model.expect("model"),
            serial_number: "".into(),
            firmware_version: firmware_version.expect("firmware version"),
            firmware_build_date: None,
            architecture: None,
            soc: None,
            hardware_id: None,
        };

        let root = groups
            .iter()
            .find(|g| g.name == "root")
            .expect("root group");

        let properties = root.group("Properties").expect("Properties group");

        let firmware = properties.group("Firmware").expect("Firmware group");
        for param in &firmware.parameters {
            if let "BuildDate" = param.name.as_str() {
                metadata.firmware_build_date = param.current_value.clone()
            }
        }

        let system = properties.group("System").expect("System group");
        for param in &system.parameters {
            match param.name.as_str() {
                "Architecture" => metadata.architecture = param.current_value.clone(),
                "Soc" => metadata.soc = param.current_value.clone(),
                "HardwareID" => metadata.hardware_id = param.current_value.clone(),
                "SerialNumber" => {
                    metadata.serial_number = param.current_value.clone().expect("serial number")
                }
                _ => (),
            }
        }

        if metadata.serial_number == "" {
            return Err(Error::Other("serial number unavailable"));
        }

        Ok(metadata)
    }
}
