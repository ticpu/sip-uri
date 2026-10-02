#![cfg(feature = "serde")]

use serde::{Deserialize, Serialize};
use serde_json::json;
use sip_uri::{Host, SipUri, TelUri, Uri, UriParse, UrnUri};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Row {
    #[serde(with = "sip_uri::serde_str::uri")]
    uri: Uri,
    #[serde(with = "sip_uri::serde_str::sip_uri")]
    sip: SipUri,
    #[serde(with = "sip_uri::serde_str::tel_uri")]
    tel: TelUri,
    #[serde(with = "sip_uri::serde_str::urn_uri")]
    urn: UrnUri,
    #[serde(with = "sip_uri::serde_str::host")]
    host: Host,
    #[serde(with = "sip_uri::serde_str::sip_uri::option", default)]
    contact: Option<SipUri>,
    #[serde(with = "sip_uri::serde_str::host::option", default)]
    via: Option<Host>,
}

#[test]
fn values_read_and_write_as_text() {
    let text = json!({
        "uri": "urn:service:sos",
        "sip": "SIP:alice@EXAMPLE.com;transport=tcp",
        "tel": "tel:+15551234567;cpc=emergency",
        "urn": "urn:SERVICE:sos.fire",
        "host": "[2001:db8::1]",
        "contact": "sip:bob@198.51.100.1:5060",
        "via": null,
    });
    let row: Row = serde_json::from_value(text).unwrap();
    assert_eq!(
        row.sip,
        SipUri::parse("sip:alice@example.com;transport=tcp").unwrap()
    );
    assert_eq!(row.via, None);
    assert_eq!(
        serde_json::to_value(&row).unwrap(),
        json!({
            "uri": "urn:service:sos",
            "sip": "sip:alice@example.com;transport=tcp",
            "tel": "tel:+15551234567;cpc=emergency",
            "urn": "urn:service:sos.fire",
            "host": "[2001:db8::1]",
            "contact": "sip:bob@198.51.100.1:5060",
            "via": null,
        })
    );
    let again: Row = serde_json::from_value(serde_json::to_value(&row).unwrap()).unwrap();
    assert_eq!(again, row);
}

#[test]
fn missing_option_is_none() {
    let row: Row = serde_json::from_value(json!({
        "uri": "https://example.com",
        "sip": "sip:example.com",
        "tel": "tel:+15551234567",
        "urn": "urn:service:sos",
        "host": "example.com",
    }))
    .unwrap();
    assert_eq!(row.contact, None);
    assert_eq!(
        row.uri
            .as_other()
            .map(|o| o.as_str()),
        Some("https://example.com")
    );
}

#[test]
fn parse_errors_name_no_input() {
    #[derive(Debug, Deserialize)]
    struct OnlyTel {
        #[serde(with = "sip_uri::serde_str::tel_uri")]
        #[allow(dead_code)]
        tel: TelUri,
    }
    let err = serde_json::from_value::<OnlyTel>(json!({"tel": "sip:+15551234567@example.com"}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("scheme belongs to another URI type"), "{err}");
    assert!(!err.contains("5551234567"), "{err}");
}

#[test]
fn type_errors_name_no_value() {
    use serde_json::{Error, Value};
    use sip_uri::serde_str;

    type Adapter = fn(Value) -> Result<(), Error>;
    let adapters: [(&str, Adapter); 10] = [
        ("uri", |v| serde_str::uri::deserialize(v).map(drop)),
        ("sip_uri", |v| serde_str::sip_uri::deserialize(v).map(drop)),
        ("tel_uri", |v| serde_str::tel_uri::deserialize(v).map(drop)),
        ("urn_uri", |v| serde_str::urn_uri::deserialize(v).map(drop)),
        ("host", |v| serde_str::host::deserialize(v).map(drop)),
        ("uri::option", |v| {
            serde_str::uri::option::deserialize(v).map(drop)
        }),
        ("sip_uri::option", |v| {
            serde_str::sip_uri::option::deserialize(v).map(drop)
        }),
        ("tel_uri::option", |v| {
            serde_str::tel_uri::option::deserialize(v).map(drop)
        }),
        ("urn_uri::option", |v| {
            serde_str::urn_uri::option::deserialize(v).map(drop)
        }),
        ("host::option", |v| {
            serde_str::host::option::deserialize(v).map(drop)
        }),
    ];
    let values = [
        (json!(15551234567_u64), "15551234567"),
        (json!(true), "true"),
        (json!({"user": "secret-key"}), "secret-key"),
        (json!(["secret-item"]), "secret-item"),
    ];
    for (name, adapter) in adapters {
        for (value, quoted) in &values {
            let err = adapter(value.clone())
                .unwrap_err()
                .to_string();
            assert!(err.contains("expected URI text"), "{name}: {err}");
            assert!(!err.contains(quoted), "{name}: {err}");
        }
    }
}
