//! The [disk management API](https://www.axis.com/vapix-library/subjects/t10037719/section/t10004596/display?section=t10004596-t10004496).

use crate::{Device, Error, Transport};
use serde::Deserialize;

/// The disk management API.
pub struct DiskManagement<'a, T: Transport>(&'a Device<T>, String);

impl<'a, T: Transport> DiskManagement<'a, T> {
    pub(crate) fn new(device: &'a Device<T>, api_version: String) -> Self {
        Self(device, api_version)
    }

    /// List disks provided by the device.
    pub async fn list(&self) -> Result<Vec<DiskInfo>, Error<T::Error>> {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri(
                self.0
                    .uri_for("/axis-cgi/disks/list.cgi?diskid=all")
                    .unwrap(),
            )
            .body(Vec::new())
            .unwrap();

        let (_resp, resp_body) = self.0.roundtrip(req, "text/xml").await?;

        let resp_body =
            std::str::from_utf8(resp_body.as_slice()).map_err(|_| Error::Other("invalid UTF-8"))?;

        #[derive(Deserialize)]
        struct ListResponse {
            #[serde(rename = "disks")]
            container: Container,
        }
        #[derive(Deserialize)]
        struct Container {
            #[serde(rename = "disk")]
            disks: Vec<DiskInfo>,
        }
        let resp_body: ListResponse = quick_xml::de::from_str(resp_body)?;

        Ok(resp_body.container.disks)
    }
}

/// Information about a disk.
///
/// A disk may be physically connected like an SD card, or it may be a network share.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
pub struct DiskInfo {
    /// The identifier for this disk.
    #[serde(rename = "diskid")]
    pub disk_id: String,

    /// The filesystem label, if any.
    pub name: String,

    /// The formatted size of the disk in bytes.
    #[serde(rename = "totalsize")]
    pub total_size: u64,

    /// The free space of the disk in bytes.
    #[serde(rename = "freesize")]
    pub free_size: u64,

    /// Purpose unknown.
    #[serde(rename = "cleanuplevel")]
    pub cleanup_level: u16, //"99"

    /// The maximum age of a recording on this disk.
    #[serde(rename = "cleanupmaxage")]
    pub cleanup_max_age: u16,

    /// The cleanup policy for this disk.
    #[serde(rename = "cleanuppolicy")]
    pub cleanup_policy: CleanupPolicy,

    /// TODO
    #[serde(deserialize_with = "deserialize_yesno")]
    pub locked: bool, //"no"

    /// TODO
    pub full: bool, //"no"

    /// TODO
    pub readonly: bool, //"no"

    pub status: String, //"OK"

    pub filesystem: Filesystem, //"ext4"

    pub group: String, //"S0"

    #[serde(rename = "requiredfilesystem")]
    pub required_filesystem: Filesystem,

    #[serde(rename = "encryptionenabled")]
    pub encryption_enabled: bool,

    #[serde(rename = "diskencrypted")]
    pub disk_encrypted: bool,
}

/// The cleanup policy for a disk.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize)]
pub enum CleanupPolicy {
    /// First in, first out.
    #[serde(rename = "fifo")]
    FIFO,
    /// No automatic cleanups will be performed.
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize)]
pub enum Filesystem {
    #[serde(rename = "ext4")]
    EXT4,
    #[serde(rename = "vfat")]
    VFAT,
    #[serde(rename = "cifs")]
    CIFS,
    #[serde(rename = "none")]
    None,
}

fn deserialize_yesno<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = bool;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str(r#""yes" or "no""#)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            match v {
                "yes" => Ok(true),
                "no" => Ok(false),
                other => Err(E::invalid_value(serde::de::Unexpected::Str(other), &self)),
            }
        }
    }

    d.deserialize_any(V)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list() {
        crate::test_with_devices(|test_device| async move {
            let services = test_device.device.services().await?;
            let disk_management = services.disk_management.ok_or(Error::UnsupportedFeature)?;

            // disk list should always be non-empty
            let disks = disk_management.list().await?;
            assert_ne!(disks.len(), 0);

            Ok(())
        });
    }

    #[test]
    fn deserialize_list() {
        #[derive(Deserialize)]
        struct ListResponse {
            #[serde(rename = "disks")]
            container: Container,
        }
        #[derive(Deserialize)]
        struct Container {
            #[serde(rename = "disk")]
            disks: Vec<DiskInfo>,
        }

        let ListResponse { container } = quick_xml::de::from_str(r#"<?xml version="1.0"?>
<root xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="http://www.axis.com/vapix/http_cgi/disk/list1.xsd">
    <disks numberofdisks="2">
        <disk diskid="SD_DISK" name="" totalsize="116109036" freesize="75106020" cleanuplevel="99" cleanupmaxage="7" cleanuppolicy="fifo" locked="no" full="no" readonly="no" status="OK" filesystem="ext4" group="S0" requiredfilesystem="none" encryptionenabled="false" diskencrypted="false"/>
        <disk diskid="NetworkShare" name="" totalsize="0" freesize="0" cleanuplevel="90" cleanupmaxage="7" cleanuppolicy="fifo" locked="no" full="no" readonly="no" status="disconnected" filesystem="cifs" group="S1" requiredfilesystem="none" encryptionenabled="false" diskencrypted="false"/>
    </disks>
</root>
"#).unwrap();
        assert_eq!(container.disks.len(), 2);
        assert_eq!(
            container.disks[0],
            DiskInfo {
                disk_id: "SD_DISK".to_string(),
                name: "".to_string(),
                total_size: 116109036,
                free_size: 75106020,
                cleanup_level: 99,
                cleanup_max_age: 7,
                cleanup_policy: CleanupPolicy::FIFO,
                locked: false,
                full: false,
                readonly: false,
                status: "OK".to_string(),
                filesystem: Filesystem::EXT4,
                group: "S0".to_string(),
                required_filesystem: Filesystem::None,
                encryption_enabled: false,
                disk_encrypted: false,
            }
        );
    }
}
