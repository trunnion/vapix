//! The VAPIX v3 parameters interface at `/axis-cgi/param.cgi`.

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

/// A device's legacy parameters API.
pub struct Parameters<'a, T: Transport>(&'a Client<T>, String);

impl<'a, T: Transport> Parameters<'a, T> {
    pub(crate) fn new(device: &'a Client<T>, api_version: String) -> Self {
        Self(device, api_version)
    }

    /// List parameters, including their definitions and current values.
    ///
    /// If `groups` is provided, return a subset of the parameter tree.
    pub async fn list_definitions(&self, groups: Option<&[&str]>) -> Result<ParameterDefinitions> {
        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri(
                self.0
                    .uri_for_args(
                        "/axis-cgi/param.cgi",
                        ListParams {
                            action: "listdefinitions",
                            list_format: Some("xmlschema"),
                            groups,
                        },
                    )
                    .unwrap(),
            )
            .body(Vec::new())
            .unwrap();

        let (_resp, resp_body) = self.0.roundtrip(req, "text/xml").await?;

        let resp_body =
            std::str::from_utf8(resp_body.as_slice()).map_err(|_| Error::Other("invalid UTF-8"))?;

        let params: ParameterDefinitions = quick_xml::de::from_str(resp_body)?;

        Ok(params)
    }

    /// List parameters, including their current values.
    ///
    /// If `groups` is provided, return a subset of the parameter tree.
    pub async fn list(&self, groups: Option<&[&str]>) -> Result<BTreeMap<String, String>> {
        let req = http::request::Builder::new()
            .method(http::Method::GET)
            .uri(
                self.0
                    .uri_for_args(
                        "/axis-cgi/param.cgi",
                        ListParams {
                            action: "list",
                            list_format: None,
                            groups,
                        },
                    )
                    .unwrap(),
            )
            .body(Vec::new())
            .unwrap();

        let (_, body) = self.0.roundtrip(req, "text/plain").await?;
        Ok(body
            .as_slice()
            .split(|byte| *byte == b'\n')
            .filter_map(|line| {
                let line = std::str::from_utf8(line).unwrap_or("");
                let mut parts = line.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                    _ => None,
                }
            })
            .collect())
    }

    // todo: ?action=add, optional force=yes
    // The force parameter can be used to exceed limits set for adding dynamic parameter groups.
    // Example: Axis products can be configured for up to 10 event types. The force parameter can be used to exceed this maximum number of events.

    // todo action=remove

    /// Attempt to update one or more parameters.
    ///
    /// TODO: what happens with partial failure?
    pub async fn update<I: IntoIterator<Item = (K, V)>, K: AsRef<str>, V: AsRef<str>>(
        &self,
        parameters: I,
    ) -> Result<()> {
        let mut query_params: BTreeMap<String, String> = parameters
            .into_iter()
            .map(move |(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
            .collect();
        query_params.insert("action".into(), "update".into());

        assert!(!query_params.is_empty());

        let req = http::request::Builder::new()
            .method(http::Method::GET)
            .uri(
                self.0
                    .uri_for_args("/axis-cgi/param.cgi", query_params)
                    .unwrap(),
            )
            .body(Vec::new())
            .unwrap();

        let (_, body) = self.0.roundtrip(req, "text/plain").await?;
        if body.as_slice() == b"OK" {
            Ok(())
        } else if body.as_slice().starts_with(b"# ") {
            // xxx: body contains error message
            Err(Error::Other("call failed for specific reason"))
        } else {
            Err(Error::Other("call failed for unknown reason"))
        }
    }
}

#[derive(Serialize)]
struct ListParams<'a> {
    action: &'a str,
    #[serde(skip_serializing_if = "Option::is_none", rename = "listformat")]
    list_format: Option<&'a str>,
    #[serde(
        rename = "group",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_list_params_groups"
    )]
    groups: Option<&'a [&'a str]>,
}

fn serialize_list_params_groups<S>(groups: &Option<&[&str]>, ser: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match groups {
        Some(groups) => {
            let groups = groups.join(",");
            ser.serialize_str(&groups)
        }
        None => unreachable!(),
    }
}

/// A set of parameter definitions.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterDefinitions {
    /// The version of the data structures used to describe the parameter definitions.
    ///
    /// In practice, always `"1.0"`.
    #[serde(rename = "version")]
    pub schema_version: String,

    /// The name of the device model.
    pub model: Option<String>,

    /// The version of firmware running on the device.
    pub firmware_version: Option<String>,

    /// Parameter groups provided by this device.
    #[serde(rename = "group")]
    pub groups: Vec<ParameterGroupDefinition>,
}

/// A group of parameter definitions.
///
/// May contain parameters or additional groups.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterGroupDefinition {
    /// The name of the parameter group.
    pub name: String,

    /// Purpose unknown.
    pub max_groups: Option<u32>,

    /// The parameter groups nested within this parameter group.
    #[serde(rename = "group", default)]
    pub groups: Vec<ParameterGroupDefinition>,

    /// The parameters nested within this parameter group.
    #[serde(rename = "parameter", default)]
    pub parameters: Vec<ParameterDefinition>,
}

impl ParameterGroupDefinition {
    /// Find a nested parameter group by name.
    pub fn group(&self, name: &str) -> Option<&ParameterGroupDefinition> {
        self.groups.iter().find(|g| g.name == name)
    }

    /// Find a nested parameter by name.
    pub fn parameter(&self, name: &str) -> Option<&ParameterDefinition> {
        self.parameters.iter().find(|g| g.name == name)
    }
}

/// A parameter definition.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterDefinition {
    /// The name of the parameter.
    pub name: String,

    /// The current value of the parameter, if any, expressed as a string.
    #[serde(rename = "value")]
    pub current_value: Option<String>,

    /// The security level of the parameter.
    pub security_level: Option<u32>, // FIXME: this is a 4-digit octal string. What does it mean?

    /// The name to display to the user, if different from `name`.
    pub nice_name: Option<String>,

    /// The type of this parameter, if provided.
    #[serde(rename = "type")]
    pub parameter_type: Option<ParameterTypeDefinition>,
}

impl ParameterDefinition {
    /// Return this parameter as a `bool`. Returns `None` if this parameter has no `current_value`,
    /// has no `parameter_type`, has a `parameter_type` with a `type_definition` other than
    /// `TypeDefinition::Bool`, or if `current_value` is neither `true_value` nor `false_value`.
    pub fn as_bool(&self) -> Option<bool> {
        match (self.current_value.as_ref(), self.parameter_type.as_ref()) {
            (
                Some(value),
                Some(ParameterTypeDefinition {
                    type_definition: TypeDefinition::Bool(td),
                    ..
                }),
            ) => {
                if value == &td.true_value {
                    Some(true)
                } else if value == &td.false_value {
                    Some(false)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct SecurityLevel {
    pub create: AccessLevel,
    pub delete: AccessLevel,
    pub read: AccessLevel,
    pub write: AccessLevel,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum BadSecurityLevelError {
    BadAccessLevel(BadAccessLevelError),
    WrongLength(String),
}

impl fmt::Display for BadSecurityLevelError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BadSecurityLevelError::BadAccessLevel(BadAccessLevelError(c)) => {
                write!(f, "expected access level digit, got '{}'", c)
            }
            BadSecurityLevelError::WrongLength(str) => {
                write!(f, "expected 4 digits, got {:?}", str)
            }
        }
    }
}

impl From<BadAccessLevelError> for BadSecurityLevelError {
    fn from(l: BadAccessLevelError) -> Self {
        BadSecurityLevelError::BadAccessLevel(l)
    }
}

impl FromStr for SecurityLevel {
    type Err = BadSecurityLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        fn next(
            s: &str,
            chars: &mut std::str::Chars,
        ) -> Result<AccessLevel, BadSecurityLevelError> {
            chars
                .next()
                .ok_or_else(|| BadSecurityLevelError::WrongLength(s.into()))
                .and_then(|c| AccessLevel::try_from(c).map_err(|e| e.into()))
        }

        let security_level = Self {
            create: next(s, &mut chars)?,
            delete: next(s, &mut chars)?,
            read: next(s, &mut chars)?,
            write: next(s, &mut chars)?,
        };

        if chars.next().is_none() {
            Ok(security_level)
        } else {
            Err(BadSecurityLevelError::WrongLength(s.into()))
        }
    }
}

impl fmt::Display for SecurityLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use std::fmt::Write;
        f.write_char(self.create.into())?;
        f.write_char(self.delete.into())?;
        f.write_char(self.read.into())?;
        f.write_char(self.write.into())
    }
}

impl<'de> serde::de::Deserialize<'de> for SecurityLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum AccessLevel {
    /// Not subject to access control.
    Unprotected,
    /// Accessible to viewers, operators, or administrators.
    ViewerAccess,
    /// Accessible to operators or administrators.
    OperatorAccess,
    /// Accessible to administrators.
    AdministratorAccess,
    /// Root access. Internal parameters that can be changed by firmware applications or by root
    /// editing the configuration files directly.
    RootAccess,
}

impl From<AccessLevel> for char {
    fn from(al: AccessLevel) -> Self {
        match al {
            AccessLevel::Unprotected => '0',
            AccessLevel::ViewerAccess => '1',
            AccessLevel::OperatorAccess => '4',
            AccessLevel::AdministratorAccess => '6',
            AccessLevel::RootAccess => '7',
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct BadAccessLevelError(char);

impl TryFrom<char> for AccessLevel {
    type Error = BadAccessLevelError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '0' => Ok(AccessLevel::Unprotected),
            '1' => Ok(AccessLevel::ViewerAccess),
            '4' => Ok(AccessLevel::OperatorAccess),
            '6' => Ok(AccessLevel::AdministratorAccess),
            '7' => Ok(AccessLevel::RootAccess),
            other => Err(BadAccessLevelError(other)),
        }
    }
}

/// A parameter type definition, describing flags and type information.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterTypeDefinition {
    /// Is this parameter read-only?
    #[serde(rename = "readonly")]
    pub read_only: Option<bool>,

    /// Is this parameter write-only?
    #[serde(rename = "writeonly")]
    pub write_only: Option<bool>,

    /// Should this parameter be displayed?
    pub hidden: Option<bool>,

    /// Is this parameter constant?
    ///
    /// (FIXME: How does this differ from `read_only`?)
    #[serde(rename = "const")]
    pub constant: Option<bool>,

    /// Purpose unknown.
    #[serde(rename = "nosync")]
    pub no_sync: Option<bool>,

    /// Purpose unknown.
    pub internal: Option<bool>,

    /// The type definition of this parameter, describing its domain and encoding.
    #[serde(rename = "$value")]
    pub type_definition: TypeDefinition,
}

/// A type definition, describing a parameter's domain and encoding.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TypeDefinition {
    /// A string, to be displayed as a text box.
    String(StringParameterDefinition),
    /// A string, to be displayed as a password box.
    Password(PasswordParameterDefinition),
    /// An integer, to be displayed as a text box.
    Int(IntParameterDefinition),
    /// An enumeration, to be displayed as a select box.
    Enum(EnumParameterDefinition),
    /// A boolean, to be displayed as a select box.
    Bool(BoolParameterDefinition),
    /// An IP address.
    ///
    /// FIXME: IPv4 or IPv6?
    Ip,
    /// A list of IP addresses.
    ///
    /// FIXME: encoding?
    IpList,
    /// A hostname.
    ///
    /// FIXME: details?
    Hostname,
    /// A string, to be displayed as a multiline text box.
    TextArea,
}

/// String parameter definition details.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringParameterDefinition {
    /// The maximum length of the string.
    #[serde(rename = "maxlen")]
    pub max_len: Option<u32>,
}

/// Password parameter definition details.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordParameterDefinition {
    /// The maximum length of the string.
    #[serde(rename = "maxlen")]
    pub max_len: Option<u32>,
}

/// Integer parameter definition details.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntParameterDefinition {
    /// The minimum value of the integer.
    pub min: Option<i64>,
    /// The maximum value of the integer.
    pub max: Option<i64>,
    /// The maximum length of the integer as a string.
    #[serde(rename = "maxlen")]
    pub max_len: Option<u8>,
    /// Range(s) in which the integer must be contained.
    pub range_entries: Option<Vec<IntParameterRangeDefinition>>,
}

/// Integer parameter range definiton details.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntParameterRangeDefinition {
    /// TODO: parse "0" and "1024-65534" into something more appropriate
    pub value: String,
}

/// Enumeration parameter definition details.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumParameterDefinition {
    /// A list of entries from which the parameter value must be selected.
    #[serde(rename = "entry")]
    pub values: Vec<EnumEntryDefinition>,
}

/// An enumeration entry.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumEntryDefinition {
    /// The value of the parameter.
    pub value: String,
    /// The value to display to the user, if different from `value`.
    pub nice_value: Option<String>,
}

/// Boolean parameter definition details.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoolParameterDefinition {
    /// The string value used to represent `true`.
    #[serde(rename = "true")]
    pub true_value: String,
    /// The string value used to represent `false`.
    #[serde(rename = "false")]
    pub false_value: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn list() {
        crate::test_with_devices(|test_device| async move {
            let parameters = test_device.client.parameters();

            let all_params = parameters.list(None).await?;

            let brand_params = parameters.list(Some(&["root.Brand"])).await?;
            let brand_and_firmware_params = parameters
                .list(Some(&["root.Brand", "root.Properties.Firmware"]))
                .await?;

            assert!(all_params.len() > 0);

            assert!(
                all_params.len() > brand_params.len(),
                "all_params.len() = {} is not greater than brand_params.len() = {}",
                all_params.len(),
                brand_params.len()
            );
            assert!(
                all_params.len() > brand_and_firmware_params.len(),
                "all_params.len() = {} is not greater than brand_and_firmware_params.len() = {}",
                all_params.len(),
                brand_and_firmware_params.len()
            );
            assert!(
                brand_and_firmware_params.len() > brand_params.len(),
                "brand_and_firmware_params.len() = {} is not greater than brand_params.len() = {}",
                brand_and_firmware_params.len(),
                brand_params.len()
            );

            Ok(())
        });
    }

    #[test]
    fn list_definitions() {
        crate::test_with_devices(|test_device| async move {
            let parameters = test_device.client.parameters();

            let all_params = parameters.list_definitions(None).await?;

            let brand_params = parameters.list_definitions(Some(&["root.Brand"])).await?;
            let brand_and_firmware_params = parameters
                .list_definitions(Some(&["root.Brand", "root.Properties.Firmware"]))
                .await?;

            assert_eq!(all_params.groups.len(), 1);
            assert_eq!(brand_params.groups.len(), 1);
            assert_eq!(brand_and_firmware_params.groups.len(), 1);

            assert_eq!(all_params.model, brand_params.model);
            assert_eq!(all_params.model, brand_and_firmware_params.model);

            assert_eq!(all_params.firmware_version, brand_params.firmware_version);
            assert_eq!(
                all_params.firmware_version,
                brand_and_firmware_params.firmware_version
            );

            assert!(all_params.groups[0].groups.len() > 2);
            assert_eq!(brand_params.groups[0].groups.len(), 1);
            assert_eq!(brand_and_firmware_params.groups[0].groups.len(), 2);

            Ok(())
        });
    }

    #[tokio::test]
    async fn update() {
        let device = crate::mock_client(|req| {
            assert_eq!(req.method(), http::Method::GET);
            assert_eq!(
                req.uri().path_and_query().map(|pq| pq.as_str()),
                Some("/axis-cgi/param.cgi?action=update&foo.bar=baz+quxx")
            );

            http::Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "text/plain")
                .body(vec![b"OK".to_vec()])
        });

        let response = device
            .parameters()
            .update(vec![("foo.bar", "baz quxx")])
            .await;
        match response {
            Ok(()) => {}
            Err(e) => panic!("update should succeed: {}", e),
        };

        let device = crate::mock_client(|req| {
            assert_eq!(req.method(), http::Method::GET);
            assert_eq!(
                req.uri().path_and_query().map(|pq| pq.as_str()),
                Some("/axis-cgi/param.cgi?action=update&foo.bar=baz+quxx")
            );

            http::Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "text/plain")
                .body(vec![
                    b"# Error: Error setting 'foo.bar' to 'baz quxx'!".to_vec()
                ])
        });

        let response = device
            .parameters()
            .update(vec![("foo.bar", "baz quxx")])
            .await;
        match response {
            Err(crate::Error::Other(_)) => {}
            Ok(()) => panic!("update should fail"),
            Err(e) => panic!("update should fail with a different error: {}", e),
        };
    }
}
