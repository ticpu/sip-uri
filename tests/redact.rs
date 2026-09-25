use sip_uri::{Redaction, SipUri, TelUri, Uri, UriParse, UriRedact, UserMask};

fn sip(s: &str) -> SipUri {
    SipUri::parse(s).unwrap()
}

#[test]
fn default_masks_whole_userinfo() {
    let uri = sip("sips:+15551234567;cpc=emergency:secret@example.com:5061;user=phone?Subject=x");
    assert_eq!(
        uri.redacted(Redaction::default())
            .to_string(),
        "sips:***@example.com:5061;user=phone?Subject=x"
    );
}

#[test]
fn user_mask_reads_back() {
    assert_eq!(Redaction::default().user_mask(), UserMask::Full);
    assert_eq!(
        Redaction::default()
            .user(UserMask::KeepLast(4))
            .user_mask(),
        UserMask::KeepLast(4)
    );
}

#[test]
fn default_leaves_uri_without_userinfo_unchanged() {
    let uri = sip("sip:example.com;transport=tcp");
    assert_eq!(
        uri.redacted(Redaction::default())
            .to_string(),
        uri.to_string()
    );
}

#[test]
fn visible_user_still_masks_password() {
    let uri = sip("sip:alice:secret@example.com");
    assert_eq!(
        uri.redacted(Redaction::default().user(UserMask::Visible))
            .to_string(),
        "sip:alice:***@example.com"
    );
}

#[test]
fn keep_last_masks_only_digits() {
    let how = Redaction::default().user(UserMask::KeepLast(4));
    assert_eq!(
        sip("sip:+1-555-123-4567@example.com")
            .redacted(how)
            .to_string(),
        "sip:+x-xxx-xxx-4567@example.com"
    );
    assert_eq!(
        sip("sip:12@example.com")
            .redacted(how)
            .to_string(),
        "sip:12@example.com"
    );
}

#[test]
fn headers_and_named_params_can_be_masked() {
    let uri = sip("sip:alice@example.com;participantid=abc;user=phone?Replaces=x%40y");
    let how = Redaction::default()
        .user(UserMask::Visible)
        .drop_headers()
        .params(&["ParticipantId"]);
    assert_eq!(
        uri.redacted(how)
            .to_string(),
        "sip:alice@example.com;participantid=***;user=phone"
    );
}

#[test]
fn tel_number_is_masked_by_default() {
    let tel = TelUri::parse("tel:+15551234567;cpc=emergency").unwrap();
    assert_eq!(
        tel.redacted(Redaction::default())
            .to_string(),
        "tel:***;cpc=emergency"
    );
    assert_eq!(
        tel.redacted(Redaction::default().user(UserMask::KeepLast(4)))
            .to_string(),
        "tel:+xxxxxxx4567;cpc=emergency"
    );
}

#[test]
fn uri_dispatches_and_leaves_others_unmasked() {
    let sip = Uri::parse("sip:alice@example.com").unwrap();
    assert_eq!(
        sip.redacted(Redaction::default())
            .to_string(),
        "sip:***@example.com"
    );
    let urn = Uri::parse("urn:service:sos").unwrap();
    assert_eq!(
        urn.redacted(Redaction::default())
            .to_string(),
        "urn:service:sos"
    );
}
