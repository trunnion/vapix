//! The [basic device info API](https://www.axis.com/vapix-library/subjects/t10037719/section/t10132180/display).

use crate::v4::JsonService;
use crate::*;
use serde::{Deserialize, Serialize};

/// The basic device info API.
pub struct BasicDeviceInfo<'a, T: Transport>(JsonService<'a, T>);

/// A set of basic device properties.
#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
pub struct Properties {
    /// The brand of the device, likely `"AXIS"`.
    #[serde(rename = "Brand")]
    pub brand: String,
    /// The hardware ID, believed to be an internal AXIS part identifier.
    #[serde(rename = "HardwareID")]
    pub hardware_id: String,
    /// The full name of the product.
    #[serde(rename = "ProdFullName")]
    pub product_full_name: String,
    /// The product number seen in typical product catalogs.
    #[serde(rename = "ProdNbr")]
    pub product_number: String,
    /// The short name of the product.
    #[serde(rename = "ProdShortName")]
    pub product_short_name: String,
    /// TODO
    #[serde(rename = "ProdType")]
    pub product_type: String,
    /// TODO
    #[serde(rename = "ProdVariant")]
    pub product_variant: String,
    /// The device's serial number.
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    /// The name of the system-on-chip inside the device.
    #[serde(rename = "Soc")]
    pub soc: String,
    /// The architecture of the system-on-chip inside the device.
    #[serde(rename = "Architecture")]
    pub soc_architecture: String,
    /// The system-on-chip's serial number.
    #[serde(rename = "SocSerialNumber")]
    pub soc_serial_number: String,
    /// The firmware's build date, expressed as a string.
    #[serde(rename = "BuildDate")]
    pub firmware_build_date: String,
    /// The firmware version, expressed as a string.
    #[serde(rename = "Version")]
    pub firmware_version: String,
    /// TOOD
    #[serde(rename = "WebURL")]
    pub web_url: String,
}

impl<'a, T: Transport> BasicDeviceInfo<'a, T> {
    pub(crate) fn new(client: &'a Client<T>, api_version: String) -> Self {
        Self(JsonService::new(
            client,
            "/axis-cgi/basicdeviceinfo.cgi",
            api_version,
        ))
    }

    /// Retreive `Properties`.
    pub async fn properties(&self) -> Result<Properties> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Req<'a> {
            property_list: &'a [&'a str],
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            property_list: Properties,
        }

        let resp: Resp = self
            .0
            .call_method(
                "getProperties",
                Req {
                    property_list: &[
                        "Architecture",
                        "Brand",
                        "BuildDate",
                        "HardwareID",
                        "ProdFullName",
                        "ProdNbr",
                        "ProdShortName",
                        "ProdType",
                        "ProdVariant",
                        "SerialNumber",
                        "Soc",
                        "SocSerialNumber",
                        "Version",
                        "WebURL",
                    ],
                },
            )
            .await?;

        Ok(resp.property_list)
    }
}
