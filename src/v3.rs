//! The VAPIX v3 AP was introduced in firmware 5.00 in 2008.
//!
//! This API is documented in [a PDF] which does not appear to be presently be available from Axis.
//! It remains available from third parties, including [the Wayback Machine].
//!
//! The v3 API does not provide an explicit service discovery interface, although certain parts of
//! it are revealed via the v4 API. Devices which primarily rely on v4 in 2020 continue to rely on
//! v3 for certain functions.
//!
//! [a PDF]: https://www.google.com/search?q=VAPIX_3_HTTP_API_3_00.pdf
//! [the Wayback Machine]: https://web.archive.org/web/20200911141549/http://www.consultorimaterdomini.it/index.php/documenti/download-materiale-informativo/category/5-centro-associazione-mater-domini-venezia.html?download=9%3Aprova

// The v3 PDF specifies URL encoding ISO-8859-1 values:
//
// > The ISO/IEC 8859-1 character set is supported. For example, in the string ©Axis Communications
// > the copyright symbol must be replaced with %A9 and the space with %20, i.e.
// > %A9Axis%20Communications.
//
// However, the HTML+JS in more recent firmware clearly encodes UTF-8. We do the same.

pub mod applications;
pub mod parameters;
pub mod system_log;

pub use applications::Applications;
pub use parameters::Parameters;
pub use system_log::SystemLog;
