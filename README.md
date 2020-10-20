# `vapix`

Client for [AXIS Communications](https://www.axis.com/en-us) devices' VAPIX API. Bullet points:

* Async
* `#![forbid(unsafe_code)]`

Features:

* `axis::Device` monitors and controls devices running AXIS firmware >= 5.00
* `axis::Transport` decouples the library from any [`http`](https://crates.io/crates/http) implementation

Optional features:

* `goblin`: sniff `vapix::application::Architecture` from executable files
* `hyper`: HTTP via `vapix::HyperTransport` (enabled by default)

## Basic use

```rust
// Instantiate a Device using `hyper` to communicate
let uri = http::Uri::from_static("http://user:pass@1.2.3.4");
let device = vapix::Device::new(axis::HyperTransport::default(), uri);

// Probe for VAPIX APIs supported by this device
let services = device.services().await?;

// If it supports the basic device info service...
if let Some(basic_device_info) = services.basic_device_info.as_ref() {
    // ...ask for those properties
    let properties = basic_device_info.properties().await?;
    println!("product_full_name: {:?}", properties.product_full_name);
    println!("    serial_number: {:?}", properties.serial_number);
} else {
    // If not, assume it supports the legacy parameters API, and retrieve those parameters
    let parameters = device.parameters().list(None).await?;
    println!("product_full_name: {:?}", parameters["root.Brand.ProdFullName"]);
    println!("    serial_number: {:?}", parameters["root.Properties.System.Soc"]);
}
```

## Development

Use `cargo test`, `cargo check`, `cargo clippy`, `rustfmt` as usual. Open issues with issues, open PRs with changesets.

Many API tests use `crate::mock_device()`, bound to a testing `Transport` which goes to a block instead of to
the network. If you want to ensure that a function sends the right request or does the right thing with a certain
response, mock up a test which covers exactly that.

```rust
#[tokio::test]
async fn update() {
    let device = crate::mock_device(|req| {
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
}
```

Tests covering safe-to-call APIs use `create::test_with_devices()` to test against recordings of actual devices.
`test_with_devices()` takes an async block which gets called repeatedly with various `TestDevice`s, containing metadata
and a `Device`. Tests are free to fail with `Error::UnsupportedFeature`, but assertion failures or other errors will
cause the containing test to fail.

```rust
#[test]
fn test_something() {
    crate::test_with_devices(|test_device| async move {
        // This block is called with various test_devices. If the block fails for any device, the
        // test will fail. If the test depends on particular device features, the test must probe
        // for those features, and it must succeed if the feature is missing.
        // 
        // test_device.device_info.{model, firmware_version, ..} describe the test device
        // test_device.device is a crate::Device<_> and can make arbitrary calls

        let parameters = test_device.device.parameters();
        let all_params = parameters.list_definitions(None).await?;

        // assert!() things which should always be true on every device ever
        // If behavior depends on firmware version, inquire about test_device's metadata
    });
}
```

Recordings are produced using the test suite itself.  When `RECORD_DEVICE_URI` is set, each `test_with_devices()` block
is additionally run against the live device, and a fixture is produced.

```console
$ RECORD_DEVICE_URI=http://user:pass@1.2.3.4 cargo test
…
$ git status
On branch master

Untracked files:
  (use "git add <file>..." to include in what will be committed)
	fixtures/recordings/ACCC8EF7DE6B v9.80.2.2.json
$
```

It is desirable to collect recordings from devices in the field, so `test_with_devices()` blocks must avoid modifying
the device state.
