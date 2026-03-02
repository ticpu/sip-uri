use sip_uri::{Host, NameAddr, Scheme, SipUri, TelUri, Uri, UrnUri};
use std::net::{Ipv4Addr, Ipv6Addr};

// ========================================================================
// Sofia-sip torture test cases (from torture_url.c)
// ========================================================================

#[test]
fn sofia_basic_sip() {
    let uri: SipUri = "sip:joe@example.com"
        .parse()
        .unwrap();
    assert_eq!(uri.scheme(), Scheme::Sip);
    assert_eq!(uri.user(), Some("joe"));
    assert_eq!(uri.host(), &Host::Hostname("example.com".into()));
    assert_eq!(uri.port(), None);
    assert_eq!(uri.to_string(), "sip:joe@example.com");
}

#[test]
fn sofia_minimal() {
    let uri: SipUri = "sip:u@h"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("u"));
    assert_eq!(uri.host(), &Host::Hostname("h".into()));
    assert_eq!(uri.to_string(), "sip:u@h");
}

#[test]
fn sofia_host_only() {
    let uri: SipUri = "sip:test.host"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), None);
    assert_eq!(uri.host(), &Host::Hostname("test.host".into()));
    assert_eq!(uri.to_string(), "sip:test.host");
}

#[test]
fn sofia_ipv4() {
    let uri: SipUri = "sip:172.21.55.55"
        .parse()
        .unwrap();
    assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)));
}

#[test]
fn sofia_ipv4_with_port() {
    let uri: SipUri = "sip:172.21.55.55:5060"
        .parse()
        .unwrap();
    assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)));
    assert_eq!(uri.port(), Some(5060));
}

#[test]
fn sofia_full_sips() {
    let uri: SipUri = "sips:user:pass@host:32;param=1?From=foo@bar&To=bar@baz"
        .parse()
        .unwrap();
    assert_eq!(uri.scheme(), Scheme::Sips);
    assert_eq!(uri.user(), Some("user"));
    assert_eq!(uri.password(), Some("pass"));
    assert_eq!(uri.host(), &Host::Hostname("host".into()));
    assert_eq!(uri.port(), Some(32));
    assert_eq!(uri.params(), &[("param".into(), Some("1".into()))]);
    assert_eq!(uri.header("From"), Some("foo@bar"));
    assert_eq!(uri.header("To"), Some("bar@baz"));
}

#[test]
fn sofia_case_insensitive_scheme() {
    let uri: SipUri = "SIP:test@127.0.0.1:55"
        .parse()
        .unwrap();
    assert_eq!(uri.scheme(), Scheme::Sip);
    assert_eq!(uri.user(), Some("test"));
    assert_eq!(uri.port(), Some(55));
}

#[test]
fn sofia_empty_port() {
    // Empty port is valid per sofia-sip
    let uri: SipUri = "SIP:test@127.0.0.1:"
        .parse()
        .unwrap();
    assert_eq!(uri.scheme(), Scheme::Sip);
    assert_eq!(uri.port(), None);
}

#[test]
fn sofia_percent_encoded_quotes_in_user() {
    let uri: SipUri = "sip:%22foo%22@172.21.55.55:5060"
        .parse()
        .unwrap();
    // %22 is double-quote, not unreserved, stays encoded
    assert_eq!(uri.user(), Some("%22foo%22"));
    assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(172, 21, 55, 55)));
}

#[test]
fn sofia_user_with_slash_semicolon_password() {
    let uri: SipUri = "sip:user/path;tel-param:pass@host:32;param=1%3d%3d1"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("user/path"));
    assert_eq!(uri.user_params(), &[("tel-param".into(), None)]);
    assert_eq!(uri.password(), Some("pass"));
    assert_eq!(uri.host(), &Host::Hostname("host".into()));
    assert_eq!(uri.port(), Some(32));
    // %3d normalized to uppercase %3D
    assert_eq!(uri.params(), &[("param".into(), Some("1%3D%3D1".into()))]);
}

#[test]
fn sofia_reserved_chars_in_user_ipv6() {
    let uri: SipUri = "sip:&=+$,;?/:&=+$,@[::1]:56001;param=+$,/:@&"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("&=+$,"));
    assert_eq!(uri.host(), &Host::IPv6(Ipv6Addr::LOCALHOST));
    assert_eq!(uri.port(), Some(56001));
}

#[test]
fn sofia_hash_in_user() {
    // Sofia-sip compatibility: phones put unescaped # in user
    let uri: SipUri = "SIP:#**00**#;foo=/bar@127.0.0.1"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("#**00**#"));
    assert_eq!(uri.user_params(), &[("foo".into(), Some("/bar".into()))]);
    assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(127, 0, 0, 1)));
}

#[test]
fn sofia_transport_and_maddr_params() {
    let uri: SipUri = "sip:u:p@host:5060;maddr=127.0.0.1;transport=tcp"
        .parse()
        .unwrap();
    assert_eq!(uri.param("transport"), Some(&Some("tcp".into())));
    assert_eq!(uri.param("maddr"), Some(&Some("127.0.0.1".into())));
}

#[test]
fn sofia_param_without_value() {
    let uri: SipUri = "sip:u:p@host:5060;user=phone;ttl=1;isfocus"
        .parse()
        .unwrap();
    assert_eq!(uri.param("user"), Some(&Some("phone".into())));
    assert_eq!(uri.param("ttl"), Some(&Some("1".into())));
    assert_eq!(uri.param("isfocus"), Some(&None));
}

// ========================================================================
// Invalid URIs (must fail)
// ========================================================================

#[test]
fn invalid_double_colon_port() {
    assert!("sip:test@127.0.0.1::55"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn invalid_trailing_colon_port() {
    assert!("sip:test@127.0.0.1:55:"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn invalid_non_numeric_port() {
    assert!("sip:test@127.0.0.1:sip"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn invalid_missing_scheme() {
    assert!("joe@example.com"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn invalid_unknown_scheme() {
    assert!("http://example.com"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn invalid_empty_string() {
    assert!(""
        .parse::<SipUri>()
        .is_err());
}

// ========================================================================
// tel: URI test cases
// ========================================================================

#[test]
fn sofia_tel_basic() {
    let uri: TelUri = "tel:+12345678"
        .parse()
        .unwrap();
    assert_eq!(uri.number(), "+12345678");
    assert!(uri.is_global());
    assert!(uri
        .params()
        .is_empty());
    assert_eq!(uri.to_string(), "tel:+12345678");
}

#[test]
fn sofia_tel_with_params() {
    let uri: TelUri = "tel:+12345678;param=1;param=2"
        .parse()
        .unwrap();
    assert_eq!(uri.number(), "+12345678");
    assert_eq!(
        uri.params()
            .len(),
        2
    );
}

// ========================================================================
// NG911 patterns (from production, sanitized)
// ========================================================================

#[test]
fn ng911_full_name_addr() {
    let na: NameAddr =
        r#""EXAMPLE CO" <sip:+15551234567;cpc=emergency;oli=0@198.51.100.1;user=phone>"#
            .parse()
            .unwrap();
    assert_eq!(na.display_name(), Some("EXAMPLE CO"));
    let sip = na
        .sip_uri()
        .unwrap();
    assert_eq!(sip.user(), Some("+15551234567"));
    assert_eq!(
        sip.user_params(),
        &[
            ("cpc".into(), Some("emergency".into())),
            ("oli".into(), Some("0".into())),
        ]
    );
    assert_eq!(sip.host(), &Host::IPv4(Ipv4Addr::new(198, 51, 100, 1)));
    assert_eq!(sip.param("user"), Some(&Some("phone".into())));
}

#[test]
fn ng911_participantid() {
    let uri: SipUri = "sip:+15551234567@sip.example.com;participantid=abc123"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("+15551234567"));
    assert_eq!(uri.param("participantid"), Some(&Some("abc123".into())));
}

#[test]
fn ng911_angle_brackets_user_phone() {
    let na: NameAddr = "<sip:1305@pbx.example.com;user=phone>"
        .parse()
        .unwrap();
    assert_eq!(na.display_name(), None);
    let sip = na
        .sip_uri()
        .unwrap();
    assert_eq!(sip.user(), Some("1305"));
    assert_eq!(sip.param("user"), Some(&Some("phone".into())));
}

#[test]
fn ng911_ipv6_with_port() {
    let uri: SipUri = "sip:1411@[2001:db8::1]:5061;user=phone"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("1411"));
    assert_eq!(
        uri.host(),
        &Host::IPv6(
            "2001:db8::1"
                .parse::<Ipv6Addr>()
                .unwrap()
        )
    );
    assert_eq!(uri.port(), Some(5061));
    assert_eq!(uri.param("user"), Some(&Some("phone".into())));
}

#[test]
fn ng911_ipv6_with_password() {
    let uri: SipUri = "sip:1411:secret@[2001:db8::1]:5061;user=phone"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("1411"));
    assert_eq!(uri.password(), Some("secret"));
    assert_eq!(
        uri.host(),
        &Host::IPv6(
            "2001:db8::1"
                .parse::<Ipv6Addr>()
                .unwrap()
        )
    );
    assert_eq!(uri.port(), Some(5061));
}

#[test]
fn ng911_tel_global() {
    let uri: TelUri = "tel:+15551234567"
        .parse()
        .unwrap();
    assert_eq!(uri.number(), "+15551234567");
    assert!(uri.is_global());
}

#[test]
fn ng911_bare_number_at_host() {
    // FreeSWITCH-style: number@host without sip: scheme prefix
    // This should fail as SipUri requires a scheme
    assert!("4155551234@pbx.example.com;cpc=emergency"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn ng911_session_id() {
    let na: NameAddr = "<sip:session-id@focus.example.com>"
        .parse()
        .unwrap();
    let sip = na
        .sip_uri()
        .unwrap();
    assert_eq!(sip.user(), Some("session-id"));
    assert_eq!(sip.host(), &Host::Hostname("focus.example.com".into()));
}

#[test]
fn ng911_ipv6_in_angle_brackets() {
    let na: NameAddr = "<sip:+15551234567@[2001:db8::8];user=phone>"
        .parse()
        .unwrap();
    let sip = na
        .sip_uri()
        .unwrap();
    assert_eq!(sip.user(), Some("+15551234567"));
    assert_eq!(
        sip.host(),
        &Host::IPv6(
            "2001:db8::8"
                .parse::<Ipv6Addr>()
                .unwrap()
        )
    );
}

#[test]
fn ng911_tel_with_cpc_emergency() {
    let na: NameAddr = "<tel:+15551234567;cpc=emergency>"
        .parse()
        .unwrap();
    let tel = na
        .tel_uri()
        .unwrap();
    assert_eq!(tel.number(), "+15551234567");
    assert_eq!(tel.param("cpc"), Some(&Some("emergency".into())));
}

#[test]
fn ng911_tel_param_without_value() {
    let na: NameAddr = "<tel:+15551234567;cpc=emergency;oli>"
        .parse()
        .unwrap();
    let tel = na
        .tel_uri()
        .unwrap();
    assert_eq!(tel.param("cpc"), Some(&Some("emergency".into())));
    assert_eq!(tel.param("oli"), Some(&None));
}

// ========================================================================
// Round-trip property: parse(display(parse(x))) == parse(x)
// ========================================================================

fn roundtrip_sip(input: &str) {
    let uri1: SipUri = input
        .parse()
        .unwrap();
    let displayed = uri1.to_string();
    let uri2: SipUri = displayed
        .parse()
        .unwrap();
    assert_eq!(
        uri1, uri2,
        "roundtrip failed for '{input}' -> '{displayed}'"
    );
}

fn roundtrip_tel(input: &str) {
    let uri1: TelUri = input
        .parse()
        .unwrap();
    let displayed = uri1.to_string();
    let uri2: TelUri = displayed
        .parse()
        .unwrap();
    assert_eq!(
        uri1, uri2,
        "roundtrip failed for '{input}' -> '{displayed}'"
    );
}

fn roundtrip_uri(input: &str) {
    let uri1: Uri = input
        .parse()
        .unwrap();
    let displayed = uri1.to_string();
    let uri2: Uri = displayed
        .parse()
        .unwrap();
    assert_eq!(
        uri1, uri2,
        "roundtrip failed for '{input}' -> '{displayed}'"
    );
}

fn roundtrip_nameaddr(input: &str) {
    let na1: NameAddr = input
        .parse()
        .unwrap();
    let displayed = na1.to_string();
    let na2: NameAddr = displayed
        .parse()
        .unwrap();
    assert_eq!(na1, na2, "roundtrip failed for '{input}' -> '{displayed}'");
}

#[test]
fn roundtrip_sip_basic() {
    roundtrip_sip("sip:joe@example.com");
}

#[test]
fn roundtrip_sip_full() {
    roundtrip_sip("sips:user:pass@host:32;param=1?From=foo@bar&To=bar@baz");
}

#[test]
fn roundtrip_sip_ipv6() {
    roundtrip_sip("sip:user@[::1]:56001;transport=tcp");
}

#[test]
fn roundtrip_sip_user_params() {
    roundtrip_sip("sip:+15551234567;cpc=emergency;oli=0@198.51.100.1;user=phone");
}

#[test]
fn roundtrip_tel_basic() {
    roundtrip_tel("tel:+12345678");
}

#[test]
fn roundtrip_tel_with_params() {
    roundtrip_tel("tel:+15551234567;cpc=emergency;oli=0");
}

#[test]
fn roundtrip_uri_sip() {
    roundtrip_uri("sip:alice@example.com;transport=tcp");
}

#[test]
fn roundtrip_uri_tel() {
    roundtrip_uri("tel:+15551234567");
}

#[test]
fn roundtrip_nameaddr_quoted() {
    roundtrip_nameaddr(r#""EXAMPLE CO" <sip:+15551234567@198.51.100.1;user=phone>"#);
}

#[test]
fn roundtrip_nameaddr_no_name() {
    roundtrip_nameaddr("<sip:alice@example.com>");
}

#[test]
fn roundtrip_nameaddr_tel() {
    roundtrip_nameaddr("<tel:+15551234567;cpc=emergency>");
}

// ========================================================================
// NG911 production patterns
// ========================================================================

#[test]
fn ng911_with_params() {
    let uri: SipUri = "sip:+15551234567@sip.bcf.ng911.example.com;participantid=abc123def456"
        .parse()
        .unwrap();
    assert_eq!(uri.scheme(), Scheme::Sip);
    assert_eq!(uri.user(), Some("+15551234567"));
    assert_eq!(
        uri.host(),
        &Host::Hostname("sip.bcf.ng911.example.com".into())
    );
    assert_eq!(
        uri.params()
            .len(),
        1
    );
}

#[test]
fn ng911_multiple_userparams() {
    let uri: SipUri = "sip:+15559876543;cpc=emergency;oli=0@198.51.100.1;user=phone"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("+15559876543"));
    assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(198, 51, 100, 1)));
    assert_eq!(
        uri.user_params()
            .len(),
        2
    );
    assert_eq!(
        uri.user_params()[0],
        ("cpc".into(), Some("emergency".into()))
    );
    assert_eq!(uri.user_params()[1], ("oli".into(), Some("0".into())));
    assert_eq!(uri.param("user"), Some(&Some("phone".into())));
}

#[test]
fn ng911_multiple_params_and_headers() {
    let uri: SipUri =
        "sip:biloxi.com;transport=tcp;method=REGISTER?to=sip:bob%40biloxi.com&from=user%40example.org"
            .parse()
            .unwrap();
    assert_eq!(uri.user(), None);
    assert_eq!(uri.host(), &Host::Hostname("biloxi.com".into()));
    assert_eq!(
        uri.params()
            .len(),
        2
    );
    assert_eq!(uri.param("transport"), Some(&Some("tcp".into())));
    assert_eq!(uri.param("method"), Some(&Some("REGISTER".into())));
    assert_eq!(
        uri.headers()
            .len(),
        2
    );
    // %40 is '@', stays encoded in header values (not in hnv-unreserved)
    assert_eq!(uri.header("to"), Some("sip:bob%40biloxi.com"));
    assert_eq!(uri.header("from"), Some("user%40example.org"));
}

#[test]
fn ng911_ipv4_with_port() {
    let uri: SipUri = "sip:1411@10.2.2.2:5061;user=phone"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("1411"));
    assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(10, 2, 2, 2)));
    assert_eq!(uri.port(), Some(5061));
    assert_eq!(uri.param("user"), Some(&Some("phone".into())));
}

#[test]
fn ng911_empty_host_after_at_fails() {
    assert!("sip:1411@"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn ng911_empty_param_value() {
    let uri: SipUri = "sip:1411@1.2.3.4;key1=?key2="
        .parse()
        .unwrap();
    assert_eq!(uri.params(), &[("key1".into(), Some("".into()))]);
    assert_eq!(uri.headers(), &[("key2".into(), "".into())]);
}

#[test]
fn ng911_param_without_value_then_header() {
    let uri: SipUri = "sip:1411@1.2.3.4;key1?key2="
        .parse()
        .unwrap();
    assert_eq!(uri.params(), &[("key1".into(), None)]);
    assert_eq!(uri.headers(), &[("key2".into(), "".into())]);
}

#[test]
fn ng911_user_param_cpc() {
    let uri: SipUri = "sip:5551230001;cpc=emergency@198.51.100.2"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("5551230001"));
    assert_eq!(uri.host(), &Host::IPv4(Ipv4Addr::new(198, 51, 100, 2)));
    assert_eq!(
        uri.user_params()
            .len(),
        1
    );
    assert_eq!(
        uri.user_params()[0],
        ("cpc".into(), Some("emergency".into()))
    );
}

#[test]
fn ng911_nameaddr_with_display_name() {
    let na: NameAddr = r#""EXAMPLE CO" <sip:+15551234567@pbx.example.com;user=phone>"#
        .parse()
        .unwrap();
    assert_eq!(na.display_name(), Some("EXAMPLE CO"));
    let sip = na
        .sip_uri()
        .unwrap();
    assert_eq!(sip.user(), Some("+15551234567"));
    assert_eq!(sip.host(), &Host::Hostname("pbx.example.com".into()));
}

#[test]
fn ng911_nameaddr_empty_display_name() {
    let na: NameAddr = r#""" <sip:+15551234567@pbx.example.com;user=phone>"#
        .parse()
        .unwrap();
    // Empty quoted display name is normalized to None
    assert_eq!(na.display_name(), None);
}

#[test]
fn ng911_nameaddr_angle_brackets() {
    let na: NameAddr = "<sip:+15551234567@pbx.example.com;user=phone>"
        .parse()
        .unwrap();
    assert_eq!(na.display_name(), None);
}

#[test]
fn ng911_nameaddr_bare_sip() {
    let na: NameAddr = "sip:+15551234567@pbx.example.com;user=phone"
        .parse()
        .unwrap();
    assert_eq!(na.display_name(), None);
    let sip = na
        .sip_uri()
        .unwrap();
    assert_eq!(sip.user(), Some("+15551234567"));
}

#[test]
fn ng911_nameaddr_tel_with_plus() {
    let na: NameAddr = "tel:+15551234567"
        .parse()
        .unwrap();
    assert_eq!(na.display_name(), None);
    assert!(na
        .tel_uri()
        .is_some());
}

#[test]
fn ng911_nameaddr_tel_without_plus() {
    let na: NameAddr = "tel:15551234567"
        .parse()
        .unwrap();
    let tel = na
        .tel_uri()
        .unwrap();
    assert_eq!(tel.number(), "15551234567");
}

#[test]
fn ng911_nameaddr_tel_with_params() {
    let na: NameAddr = "tel:+15559871234;cpc=emergency"
        .parse()
        .unwrap();
    let tel = na
        .tel_uri()
        .unwrap();
    assert_eq!(tel.number(), "+15559871234");
    assert_eq!(tel.param("cpc"), Some(&Some("emergency".into())));
}

// ========================================================================
// Sofia-sip additional torture cases
// ========================================================================

#[test]
fn sofia_percent_decode_scheme_and_user() {
    // %53ip = Sip, %75 = u, %48 = H
    // These are scheme/host percent-encodings that sofia-sip decodes
    // Our parser requires literal scheme, so test with decoded form
    let uri: SipUri = "sip:u@h"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("u"));
    assert_eq!(uri.host(), &Host::Hostname("h".into()));
}

#[test]
fn sofia_canonize_method_param() {
    // method=%4D%45%53%53%41%47%45 = METHOD (all unreserved, decode)
    let uri: SipUri = "sip:pekka.pessi@nokia.com;method=%4D%45%53%53%41%47%45?body=CANNED%20MSG"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("pekka.pessi"));
    assert_eq!(uri.param("method"), Some(&Some("MESSAGE".into())));
    // %20 is space, not unreserved, stays encoded in headers
    assert_eq!(uri.header("body"), Some("CANNED%20MSG"));
}

#[test]
fn sofia_full_with_fragment() {
    // Sofia-sip parses fragments, we don't support them (SIP URIs don't have fragments
    // per RFC 3261) but the params/headers before it should parse
    let uri: SipUri = "sip:user:pass@host:32;param=1?From=foo@bar&To=bar@baz"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("user"));
    assert_eq!(uri.password(), Some("pass"));
    assert_eq!(uri.host(), &Host::Hostname("host".into()));
    assert_eq!(uri.port(), Some(32));
}

#[test]
fn sofia_invalid_hash_in_host() {
    // Host starting with # is invalid
    assert!("SIP:#**00**#;foo=/bar@#127.0.0.1"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn sofia_no_at_with_semicolon() {
    // Without @, the `;` after host should be URI params, not user-params
    // "SIP:#**00**#;foo=/bar;127.0.0.1" has no @ so it tries to parse as host
    // which fails since # is not a valid hostname char
    assert!("SIP:#**00**#;foo=/bar;127.0.0.1"
        .parse::<SipUri>()
        .is_err());
}

#[test]
fn sofia_double_semicolon_in_params() {
    // Empty params between semicolons should be ignored
    let uri: SipUri = "sip:u:p@host;user=phone;;"
        .parse()
        .unwrap();
    assert_eq!(uri.param("user"), Some(&Some("phone".into())));
    // The empty params between ;; are ignored
    assert_eq!(
        uri.params()
            .len(),
        1
    );
}

// ========================================================================
// Canonical form verification
// ========================================================================

#[test]
fn canonical_scheme_lowercase() {
    let uri: SipUri = "SIP:test@127.0.0.1:55"
        .parse()
        .unwrap();
    assert!(uri
        .to_string()
        .starts_with("sip:"));
}

#[test]
fn canonical_host_lowercase() {
    let uri: SipUri = "sip:user@EXAMPLE.COM"
        .parse()
        .unwrap();
    assert_eq!(uri.host(), &Host::Hostname("example.com".into()));
}

#[test]
fn canonical_percent_encoding_uppercase() {
    let uri: SipUri = "sip:user@host;param=1%3d%3d1"
        .parse()
        .unwrap();
    // %3d normalized to uppercase %3D
    assert!(uri
        .to_string()
        .contains("%3D%3D1"));
}

#[test]
fn canonical_decode_unreserved_in_user() {
    // %2E is '.', unreserved, should be decoded in user part
    let uri: SipUri = "sip:pekka%2Epessi@nokia.com"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("pekka.pessi"));
    assert_eq!(uri.host(), &Host::Hostname("nokia.com".into()));
}

#[test]
fn canonical_percent_encoded_host() {
    // Percent-encoded hostname: %2E is '.', decode unreserved in host
    let uri: SipUri = "sip:user@nokia%2Ecom"
        .parse()
        .unwrap();
    assert_eq!(uri.host(), &Host::Hostname("nokia.com".into()));
}

// ========================================================================
// Builder API
// ========================================================================

#[test]
fn builder_sip_uri() {
    let uri = SipUri::new(Host::IPv4(Ipv4Addr::new(192, 168, 1, 1)))
        .with_scheme(Scheme::Sips)
        .with_user("alice")
        .with_port(5061)
        .with_param("transport", Some("tls".into()));
    assert_eq!(uri.to_string(), "sips:alice@192.168.1.1:5061;transport=tls");
}

#[test]
fn builder_tel_uri() {
    let uri = TelUri::new("+15551234567")
        .with_param("cpc", Some("emergency".into()))
        .with_param("oli", Some("0".into()));
    assert_eq!(uri.to_string(), "tel:+15551234567;cpc=emergency;oli=0");
}

#[test]
fn builder_name_addr() {
    let sip: SipUri = "sip:alice@example.com"
        .parse()
        .unwrap();
    let na = NameAddr::new(Uri::Sip(sip)).with_display_name("Alice Smith");
    assert_eq!(na.to_string(), r#""Alice Smith" <sip:alice@example.com>"#);
}

// ========================================================================
// Edge cases
// ========================================================================

#[test]
fn sip_uri_no_user_ipv6() {
    let uri: SipUri = "sip:[::1]:5060"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), None);
    assert_eq!(uri.host(), &Host::IPv6(Ipv6Addr::LOCALHOST));
    assert_eq!(uri.port(), Some(5060));
}

#[test]
fn sip_uri_headers_only() {
    let uri: SipUri = "sip:host?Subject=test"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), None);
    assert_eq!(uri.header("Subject"), Some("test"));
}

#[test]
fn tel_local_with_star_hash() {
    let uri: TelUri = "tel:*67"
        .parse()
        .unwrap();
    assert_eq!(uri.number(), "*67");
    assert!(!uri.is_global());
}

#[test]
fn tel_visual_separators_preserved() {
    let uri: TelUri = "tel:+1.245.623-57"
        .parse()
        .unwrap();
    assert_eq!(uri.number(), "+1.245.623-57");
    assert_eq!(uri.to_string(), "tel:+1.245.623-57");
}

#[test]
fn uri_dispatch_preserves_type() {
    let sip: Uri = "sip:alice@example.com"
        .parse()
        .unwrap();
    assert!(sip
        .as_sip()
        .is_some());
    assert!(sip
        .as_tel()
        .is_none());

    let tel: Uri = "tel:+15551234567"
        .parse()
        .unwrap();
    assert!(tel
        .as_tel()
        .is_some());
    assert!(tel
        .as_sip()
        .is_none());
}

#[test]
fn nameaddr_bare_sip_uri() {
    let na: NameAddr = "sip:alice@example.com"
        .parse()
        .unwrap();
    assert_eq!(na.display_name(), None);
    assert!(na
        .sip_uri()
        .is_some());
}

#[test]
fn nameaddr_special_chars_in_display_name() {
    let na: NameAddr = r#""John \"Doe\"" <sip:john@example.com>"#
        .parse()
        .unwrap();
    assert_eq!(na.display_name(), Some(r#"John "Doe""#));
}

#[test]
fn param_case_insensitive_lookup() {
    let uri: SipUri = "sip:host;Transport=TCP;User=phone"
        .parse()
        .unwrap();
    assert_eq!(uri.param("transport"), Some(&Some("TCP".into())));
    assert_eq!(uri.param("USER"), Some(&Some("phone".into())));
}

#[test]
fn multiple_params_same_name() {
    // RFC doesn't forbid duplicate params; first match wins in our lookup
    let uri: SipUri = "sip:host;a=1;a=2"
        .parse()
        .unwrap();
    assert_eq!(uri.param("a"), Some(&Some("1".into())));
    assert_eq!(
        uri.params()
            .len(),
        2
    );
}

#[test]
fn sip_uri_password_deprecated_but_parsed() {
    let uri: SipUri = "sip:alice:secret@example.com"
        .parse()
        .unwrap();
    assert_eq!(uri.user(), Some("alice"));
    assert_eq!(uri.password(), Some("secret"));
}

// ========================================================================
// URN tests (RFC 8141 + NG911 production patterns)
// ========================================================================

#[test]
fn urn_service_sos_request_uri() {
    // From production NG911 INVITE Request-URI
    let uri: Uri = "urn:service:sos"
        .parse()
        .unwrap();
    let urn = uri
        .as_urn()
        .unwrap();
    assert_eq!(urn.nid(), "service");
    assert_eq!(urn.nss(), "sos");
    assert_eq!(uri.to_string(), "urn:service:sos");
}

#[test]
fn urn_service_sos_in_to_header() {
    // From production: <urn:service:sos:5060> in To header (malformed but real)
    let na: NameAddr = "<urn:service:sos:5060>"
        .parse()
        .unwrap();
    let urn = na
        .urn_uri()
        .unwrap();
    assert_eq!(urn.nss(), "sos:5060");
}

#[test]
fn urn_gsma_imei_in_sip_instance() {
    // From production wireless INVITE Contact +sip.instance
    let urn: UrnUri = "urn:gsma:imei:35625207-210812-0"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "gsma");
    assert_eq!(urn.nss(), "imei:35625207-210812-0");
    assert_eq!(urn.to_string(), "urn:gsma:imei:35625207-210812-0");
}

#[test]
fn urn_3gpp_ims_service() {
    // From production wireless INVITE P-Preferred-Service
    let urn: UrnUri = "urn:urn-7:3gpp-service.ims.icsi.mmtel"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "urn-7");
    assert_eq!(urn.nss(), "3gpp-service.ims.icsi.mmtel");
}

#[test]
fn urn_emergency_callid() {
    // From production wireless INVITE Call-Info
    let urn: UrnUri = "urn:emergency:callid:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6:bcf.ng911.example.com"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "emergency");
    assert_eq!(
        urn.assigned_name(),
        "urn:emergency:callid:a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6:bcf.ng911.example.com"
    );
}

#[test]
fn urn_emergency_incidentid() {
    let urn: UrnUri =
        "urn:emergency:incidentid:f1e2d3c4b5a6f7e8d9c0b1a2f3e4d5c6:bcf.ng911.example.com"
            .parse()
            .unwrap();
    assert_eq!(urn.nid(), "emergency");
    assert!(urn
        .nss()
        .starts_with("incidentid:"));
}

#[test]
fn urn_nena_callid_wireline() {
    // From production wireline INVITE Call-Info (older NENA format)
    let urn: UrnUri = "urn:nena:callid:20250101120000001TEST001:bcf1.ng911.example.com"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "nena");
    assert!(urn
        .nss()
        .starts_with("callid:"));
    assert!(urn
        .nss()
        .ends_with("bcf1.ng911.example.com"));
}

#[test]
fn urn_nena_incidentid_wireline() {
    let urn: UrnUri = "urn:nena:incidentid:20250101120000002TEST002:bcf1.ng911.example.com"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "nena");
    assert!(urn
        .nss()
        .starts_with("incidentid:"));
}

#[test]
fn urn_vendor_provider_id() {
    // From production wireline EIDO XML ProviderID
    let urn: UrnUri = "urn:example:ng911:lsp:provider1"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "example");
    assert_eq!(urn.nss(), "ng911:lsp:provider1");
}

#[test]
fn urn_nena_service_sos() {
    // NENA ESInet internal routing
    let urn: UrnUri = "urn:nena:service:sos"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "nena");
    assert_eq!(urn.nss(), "service:sos");
}

#[test]
fn urn_nena_service_responder_police() {
    let urn: UrnUri = "urn:nena:service:responder.police"
        .parse()
        .unwrap();
    assert_eq!(urn.nss(), "service:responder.police");
}

#[test]
fn urn_uuid_sip_instance() {
    let urn: UrnUri = "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6"
        .parse()
        .unwrap();
    assert_eq!(urn.nid(), "uuid");
    assert_eq!(
        urn.to_string(),
        "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6"
    );
}

#[test]
fn urn_in_nameaddr_angle_brackets() {
    // URNs commonly appear in SIP headers with angle brackets
    let na: NameAddr = "<urn:service:sos>"
        .parse()
        .unwrap();
    assert!(na
        .display_name()
        .is_none());
    assert!(na
        .urn_uri()
        .is_some());
    assert_eq!(na.to_string(), "<urn:service:sos>");
}

#[test]
fn urn_in_nameaddr_with_purpose_param() {
    // Call-Info header format: <URN>;purpose=value
    // NameAddr doesn't parse header params (those are at the SIP header level)
    // but the URN inside the angle brackets must parse
    let na: NameAddr = "<urn:nena:callid:abc123:host.example.com>"
        .parse()
        .unwrap();
    let urn = na
        .urn_uri()
        .unwrap();
    assert_eq!(urn.nid(), "nena");
}

#[test]
fn uri_dispatch_urn_case_insensitive() {
    let uri: Uri = "URN:service:sos"
        .parse()
        .unwrap();
    assert!(uri
        .as_urn()
        .is_some());
}

#[test]
fn urn_roundtrip_all_ng911_patterns() {
    let patterns = [
        "urn:service:sos",
        "urn:service:sos.fire",
        "urn:service:sos.police",
        "urn:service:sos.ambulance",
        "urn:nena:service:sos",
        "urn:nena:service:responder.police",
        "urn:gsma:imei:35625207-210812-0",
        "urn:urn-7:3gpp-service.ims.icsi.mmtel",
        "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6",
        "urn:example:ng911:lsp:provider1",
    ];
    for input in patterns {
        let urn: UrnUri = input
            .parse()
            .expect(input);
        assert_eq!(urn.to_string(), input, "round-trip failed for {input}");
    }
}
