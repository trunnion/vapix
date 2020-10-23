//! The VAPIX v4 API is documented by AXIS at [the VAPIX
//! Library](https://www.axis.com/vapix-library/), principally in the [network video API
//! section](https://www.axis.com/vapix-library/subjects/t10037719/section/t10035974/display).

use crate::v3::Parameters;
use crate::{Device, Error, ResultExt, Transport};
use serde::Deserialize;

use basic_device_info::BasicDeviceInfo;
use disk_management::DiskManagement;
pub(crate) use json_service::JsonService;

pub mod basic_device_info;
pub mod disk_management;
mod json_service;

/// A list of available services supported by this device and by this library.
///
/// This data was returned by the `/axis-cgi/apidiscovery.cgi` API, added in firmware 8.50.
pub struct Services<'a, T: Transport> {
    pub parameters: Option<Parameters<'a, T>>,
    pub basic_device_info: Option<BasicDeviceInfo<'a, T>>,
    pub disk_management: Option<DiskManagement<'a, T>>,
}

impl<'a, T: Transport> Services<'a, T> {
    pub(crate) async fn new(device: &'a Device<T>) -> Result<Services<'a, T>, Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            api_list: Vec<AvailableApi>,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AvailableApi {
            pub id: String,
            pub version: String,
        }

        let resp: Resp = JsonService::new(device, "/axis-cgi/apidiscovery.cgi", "1.0".to_string())
            .call_method_bare("getApiList")
            .await
            .map_404_to_unsupported_feature()?;

        let mut services = Services {
            parameters: None,
            basic_device_info: None,
            disk_management: None,
        };

        for AvailableApi { id, version } in resp.api_list {
            match id.as_str() {
                "param-cgi" => services.parameters = Some(Parameters::new(device, version)),
                "basic-device-info" => {
                    services.basic_device_info = Some(BasicDeviceInfo::new(device, version))
                }
                "disk-management" => {
                    services.disk_management = Some(DiskManagement::new(device, version))
                }
                _ => (),
            }
        }

        Ok(services)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unsupported_feature() {
        let device = crate::test_utils::mock_device(|req| {
            assert_eq!(req.uri().path(), "/axis-cgi/apidiscovery.cgi");
            http::Response::builder()
                .status(http::StatusCode::NOT_FOUND)
                .body(vec![b"maybe this isn't an AXIS camera?".to_vec()])
        });

        match Services::new(&device).await {
            Err(Error::UnsupportedFeature) => {
                // expected result
            }
            Ok(_) => panic!("should have failed"),
            Err(e) => panic!("wrong error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn no_services() {
        let device = crate::test_utils::mock_device(|req| {
            assert_eq!(req.uri().path(), "/axis-cgi/apidiscovery.cgi");
            assert_eq!(
                req.headers()
                    .get(http::header::CONTENT_TYPE)
                    .map(|v| v.as_bytes()),
                Some("application/json".as_bytes())
            );
            assert_eq!(
                req.body().as_slice(),
                &br#"{"apiVersion":"1.0","method":"getApiList"}"#[..]
            );

            http::Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(vec![br#"{"data":{"apiList":[]}}"#.to_vec()])
        });

        let services = Services::new(&device).await.unwrap();
        assert!(services.parameters.is_none());
        assert!(services.basic_device_info.is_none());
        assert!(services.disk_management.is_none());
    }

    const TYPICAL_SERVICES_RESPONSE: &[u8] = br#"{"method": "getApiList", "apiVersion": "1.0", "data": {"apiList": [{"id": "privacy-mask", "version": "1.0", "name": "Privacy Masking", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "recording-storage-limit", "version": "1.0", "name": "Edge Recording storage limit", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "mdnssd", "version": "1.0", "name": "mDNS-SD", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "api-discovery", "version": "1.0", "name": "API Discovery Service", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "io-port-management", "version": "1.0", "name": "IO Port Management", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "stream-profiles", "version": "1.0", "name": "Stream Profiles", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "dynamicoverlay", "version": "1.0", "name": "Dynamic Overlay", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "disk-management", "version": "1.0", "name": "Edge storage Disk management", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "oak", "version": "1.0", "name": "OAK", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "mqtt-client", "version": "1.0", "name": "MQTT Client API", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "ntp", "version": "1.2", "name": "NTP", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "upnp", "version": "1.1", "name": "UPnP", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "network-settings", "version": "1.6", "name": "Network Settings", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "systemready", "version": "1.1", "name": "Systemready", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "time-service", "version": "1.0", "name": "Time API", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "disk-properties", "version": "1.1", "name": "Edge storage Disk properties", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "custom-firmware-certificate", "version": "1.0", "name": "Custom Firmware Certificate", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "recording", "version": "1.0", "name": "Edge Recording", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "basic-device-info", "version": "1.1", "name": "Basic Device Information", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "user-management", "version": "1.1", "name": "User Management", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "onscreencontrols", "version": "1.4", "name": "On-Screen Controls", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "shuttergain-cgi", "version": "2.0", "name": "Shuttergain CGI", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "packagemanager", "version": "1.4", "name": "Package Manager", "docLink": ""}, {"id": "overlayimage", "version": "1.0", "name": "Overlay image API", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "ptz-control", "version": "1.0", "name": "PTZ Control", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "capture-mode", "version": "1.0", "name": "Capture Mode", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "light-control", "version": "1.1", "name": "Light Control", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "disk-network-share", "version": "1.0", "name": "Edge storage Network share", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "recording-export", "version": "1.1", "name": "Export edge recording", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "guard-tour", "version": "1.0", "name": "Guard Tour", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "param-cgi", "version": "1.0", "name": "Legacy Parameter Handling", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "customhttpheader", "version": "1.0", "name": "Custom HTTP header", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}, {"id": "fwmgr", "version": "1.4", "name": "Firmware Management", "docLink": "https://www.axis.com/partner_pages/vapix_library/#/"}]}}
"#;

    #[tokio::test]
    async fn typical_services_response() {
        let device = crate::test_utils::mock_device(|req| {
            assert_eq!(req.uri().path(), "/axis-cgi/apidiscovery.cgi");
            assert_eq!(
                req.headers()
                    .get(http::header::CONTENT_TYPE)
                    .map(|v| v.as_bytes()),
                Some("application/json".as_bytes())
            );
            assert_eq!(
                req.body().as_slice(),
                &br#"{"apiVersion":"1.0","method":"getApiList"}"#[..]
            );

            http::Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(TYPICAL_SERVICES_RESPONSE.chunks(50).map(|c| c.to_vec()))
        });

        let services = Services::new(&device).await.unwrap();
        assert!(services.parameters.is_some());
        assert!(services.basic_device_info.is_some());
        assert!(services.disk_management.is_some());
    }
}
