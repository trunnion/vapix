//! The VAPIX application interface at `/axis-cgi/applications/*`.

use crate::*;

mod enums;
pub use enums::*;

/// A device's application management API.
pub struct Applications<'a, T: Transport> {
    device: &'a Device<T>,
    _embedded_development_version: String,
    firmware_version: Option<String>,
    soc: Option<SOC>,
    architecture: Option<Architecture>,
}

impl<'a, T: Transport> Applications<'a, T> {
    pub(crate) async fn new(device: &'a Device<T>) -> Result<Option<Applications<'a, T>>, Error> {
        let mut params = device
            .parameters()
            .list(Some(
                &[
                    "Properties.Firmware.Version",
                    "Properties.EmbeddedDevelopment.Version",
                    "Properties.System.Soc",
                    "Properties.System.Architecture",
                ][..],
            ))
            .await?;

        // If we don't have an embedded development version, we don't have a platform
        let embedded_development_version = params.remove("Properties.EmbeddedDevelopment.Version");
        let embedded_development_version = match embedded_development_version {
            Some(version) => version,
            None => return Ok(None),
        };

        let firmware_version = params.remove("Properties.Firmware.Version");
        let soc = params
            .remove("Properties.System.Soc")
            .and_then(|s| SOC::from_param(&s));
        let architecture = params
            .remove("Properties.System.Architecture")
            .and_then(|s| Architecture::from_param(&s));

        Ok(Some(Self {
            device,
            _embedded_development_version: embedded_development_version,
            firmware_version,
            soc,
            architecture,
        }))
    }

    /// The device's architecture, if known.
    pub fn architecture(&self) -> Option<Architecture> {
        self.architecture
    }

    /// The device's SOC, if known.
    pub fn soc(&self) -> Option<SOC> {
        self.soc
    }

    /// The device's firmware version, if known.
    pub fn firmware_version(&self) -> Option<&str> {
        self.firmware_version.as_ref().map(|s| s.as_ref())
    }

    /// Upload an application package to the device.
    pub async fn upload(&self, application_package_data: &[u8]) -> Result<(), Error> {
        let mut request_body = b"--fileboundary\r\n\
        Content-Disposition: form-data; name=\"packfil\"; filename=\"application.eap\"\r\n\
        Content-Type: application/octet-stream\r\n\
        \r\n"
            .to_vec();
        request_body.extend_from_slice(application_package_data);
        request_body.extend_from_slice(b"\r\n--fileboundary--\r\n\r\n");

        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri(
                self.device
                    .uri_for("/axis-cgi/applications/upload.cgi")
                    .unwrap(),
            )
            .header(
                http::header::CONTENT_TYPE,
                "multipart/form-data; boundary=fileboundary",
            )
            .header(
                http::header::CONTENT_LENGTH,
                format!("{}", request_body.len()),
            )
            .body(request_body)
            .unwrap();

        let (_resp, resp_body) = self.device.roundtrip(req, "text/plain").await?;

        let resp_body =
            std::str::from_utf8(resp_body.as_slice()).map_err(|_| Error::Other("invalid UTF-8"))?;

        if resp_body.starts_with("OK") {
            Ok(())
        } else {
            // TODO: smuggle out the error value
            Err(Error::Other("application upload failed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::v3::application::{Architecture, SOC};

    #[test]
    fn new() {
        crate::test_with_devices(|test_device| async move {
            let applications = test_device.device.applications().await?;

            let architecture = applications.as_ref().and_then(|a| a.architecture());
            let soc = applications.as_ref().and_then(|a| a.soc());

            // Keyed on Brand.ProdShortName:
            match test_device.device_info.model.as_ref() {
                "AXIS Companion Bullet LE" => {
                    assert!(applications.is_some());
                    assert_eq!(architecture, Some(Architecture::Mips));
                    assert_eq!(soc, Some(SOC::Artpec5));
                }
                "AXIS P5512" => {
                    assert!(applications.is_some());
                    assert_eq!(architecture, Some(Architecture::CrisV32));
                    assert_eq!(soc, None); // actually ARTPEC-3, but the API doesn't say
                }

                // Hello! If you're here, it's probably because you're trying to record a new device
                // model. This test requires additional information and human judgement. Please
                // determine the ground truth about your device:
                //   * https://www.axis.com/en-us/developer-community/product-interface-guide
                //   * `axctl shell`:
                //     * head /proc/cpuinfo
                //     * find /lib/modules -name '*.ko'
                //     * cat /etc/opkg/arch.conf
                //
                // Then check what the API says:
                //   * http://…/axis-cgi/param.cgi?action=list&group=Properties.System,Brand.ProdShortName
                //
                // ...and add the right test above.
                other => panic!(
                    "device model {:?} has no expectations, please update the test",
                    other
                ),
            };

            Ok(())
        })
    }
}
