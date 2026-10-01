use std::net::{Ipv4Addr, Ipv6Addr};

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::host::{Host, Hostname};
use crate::params::{Headers, Params, UserParams};
use crate::uri::OtherUri;

macro_rules! pair_list_serde {
    ($($ty:ty),*) => {$(
        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_seq(self.iter())
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Vec::<(String, Option<String>)>::deserialize(deserializer).map(<$ty>::from)
            }
        }
    )*};
}

pair_list_serde!(Params, UserParams, Headers);

/// The serialized shape of [`Host`], read before [`Host::from_hostname`].
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum HostRepr {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    Hostname(Hostname),
}

impl From<HostRepr> for Host {
    fn from(repr: HostRepr) -> Self {
        match repr {
            HostRepr::IPv4(addr) => Host::IPv4(addr),
            HostRepr::IPv6(addr) => Host::IPv6(addr),
            HostRepr::Hostname(name) => Host::from_hostname(name),
        }
    }
}

impl Serialize for Hostname {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Hostname {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Hostname::from)
    }
}

#[derive(Serialize)]
struct OtherRef<'a> {
    scheme: Option<&'a str>,
    rest: &'a str,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct OtherOwned {
    scheme: Option<String>,
    rest: String,
}

impl Serialize for OtherUri {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        OtherRef {
            scheme: self.scheme(),
            rest: self.rest(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OtherUri {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let other = OtherOwned::deserialize(deserializer)?;
        OtherUri::new(
            other
                .scheme
                .as_deref(),
            &other.rest,
        )
        .map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Host, OtherUri, SipUri, TelUri, Uri, UrnUri};
    use serde_json::json;

    fn round_trip(uri: Uri) {
        let json = serde_json::to_value(&uri).unwrap();
        assert_eq!(serde_json::from_value::<Uri>(json).unwrap(), uri);
    }

    #[test]
    fn sip_uri_serializes_as_parts() {
        let uri = SipUri::new(Host::Hostname("example.com".into()))
            .with_user("+15551234567")
            .with_param("user", Some("phone"))
            .with_header("Subject", Some("x"))
            .with_header("Flag", None);
        assert_eq!(
            serde_json::to_value(&uri).unwrap(),
            json!({
                "scheme": "sip",
                "user": "+15551234567",
                "user_params": [],
                "password": null,
                "host": {"hostname": "example.com"},
                "port": null,
                "params": [["user", "phone"]],
                "headers": [["Subject", "x"], ["Flag", null]],
                "fragment": null,
            })
        );
    }

    #[test]
    fn deserialize_canonizes() {
        let uri: SipUri = serde_json::from_value(json!({
            "user": "a b;c",
            "host": {"hostname": "EXAMPLE%2ecom"},
            "params": [["x", "a@b"], ["", null]],
            "headers": [["h", null], ["n", "a b"]],
        }))
        .unwrap();
        assert_eq!(uri.to_string(), "a%20b%3Bc@example.com;x=a%40b?h&n=a%20b");

        let tel: TelUri = serde_json::from_value(json!({"number": "+1555;x=1"})).unwrap();
        assert_eq!(tel, TelUri::new("+1555;x=1"));

        let urn: UrnUri = serde_json::from_value(json!({"nid": "SERVICE", "nss": "sos"})).unwrap();
        assert_eq!(urn, UrnUri::new("service", "sos"));

        let host: Host = serde_json::from_value(json!({"hostname": "198.51.100.1"})).unwrap();
        assert_eq!(
            host,
            Host::IPv4(
                "198.51.100.1"
                    .parse()
                    .unwrap()
            )
        );
    }

    #[test]
    fn every_variant_round_trips() {
        round_trip(
            SipUri::new(Host::IPv6(
                "2001:db8::1"
                    .parse()
                    .unwrap(),
            ))
            .with_user("alice")
            .with_user_param("cpc", Some("emergency"))
            .with_password("pw")
            .with_port(5061)
            .with_param("lr", None)
            .with_header("Subject", Some("a b"))
            .with_header("Flag", None)
            .with_fragment("f")
            .into(),
        );
        round_trip(
            SipUri::new(Host::IPv4(
                "198.51.100.1"
                    .parse()
                    .unwrap(),
            ))
            .with_user("1.2.3.4")
            .into(),
        );
        round_trip(
            TelUri::new("+15551234567")
                .with_param("cpc", Some("emergency"))
                .with_fragment("x")
                .into(),
        );
        round_trip(
            UrnUri::new("service", "sos")
                .with_r_component("r")
                .with_q_component("q")
                .with_f_component("f")
                .into(),
        );
        round_trip(Uri::Other(
            OtherUri::new(Some("HTTPS"), "//example.com").unwrap(),
        ));
        round_trip(Uri::Other(OtherUri::new(None, "*").unwrap()));
    }

    #[test]
    fn uri_is_tagged_by_kind() {
        let other = Uri::Other(OtherUri::new(Some("https"), "//example.com").unwrap());
        assert_eq!(
            serde_json::to_value(&other).unwrap(),
            json!({"other": {"scheme": "https", "rest": "//example.com"}})
        );
        let host = Host::IPv4(
            "198.51.100.1"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            serde_json::to_value(host).unwrap(),
            json!({"ipv4": "198.51.100.1"})
        );
    }

    #[test]
    fn other_uri_rejects_what_new_rejects() {
        for value in [
            json!({"scheme": "sip", "rest": "alice@example.com"}),
            json!({"scheme": "<sip", "rest": "x"}),
            json!({}),
        ] {
            let err = serde_json::from_value::<OtherUri>(value).unwrap_err();
            assert!(!err
                .to_string()
                .contains("alice"));
        }
    }
}
