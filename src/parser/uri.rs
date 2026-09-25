use crate::error::ParseError;
use crate::grammar::{self, SchemeSplit};
use crate::uri::{OtherUri, Uri};
use crate::warning::{Component, Parsed, WarningCode, Warnings};

pub(crate) fn parse(s: &str) -> Result<Parsed<Uri>, ParseError> {
    fn wrap<T>(parsed: Parsed<T>, variant: fn(T) -> Uri) -> Parsed<Uri> {
        Parsed {
            value: variant(parsed.value),
            warnings: parsed.warnings,
        }
    }

    if s.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut warnings = Warnings::new(s);
    let other = match grammar::split_scheme(s) {
        SchemeSplit::Named(scheme, _) if scheme.eq_ignore_ascii_case("tel") => {
            return Ok(wrap(super::tel::parse(s)?, Uri::Tel));
        }
        SchemeSplit::Named(scheme, _)
            if scheme.eq_ignore_ascii_case("sip") || scheme.eq_ignore_ascii_case("sips") =>
        {
            return Ok(wrap(super::sip::parse(s)?, Uri::Sip));
        }
        SchemeSplit::Named(scheme, _) if scheme.eq_ignore_ascii_case("urn") => {
            return Ok(wrap(super::urn::parse(s)?, Uri::Urn));
        }
        SchemeSplit::Named(scheme, rest) => OtherUri::new(Some(scheme), rest),
        SchemeSplit::Invalid => {
            warnings.push(Component::Scheme, WarningCode::InvalidScheme, s, 0);
            OtherUri::new(None, s)
        }
        SchemeSplit::Absent if s == "*" => {
            warnings.push(Component::Scheme, WarningCode::Wildcard, s, 0);
            OtherUri::new(None, s)
        }
        SchemeSplit::Absent => {
            warnings.push(Component::Scheme, WarningCode::MissingScheme, s, 0);
            OtherUri::new(None, s)
        }
    };
    // `split_scheme` names only known or RFC 3986 schemes, so `None` is a known one.
    let other = other.ok_or(ParseError::SchemeMismatch)?;
    Ok(warnings.finish(Uri::Other(other)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sip_uri::SipUri;
    use crate::urn_uri::UrnUri;
    use crate::UriParse;

    #[test]
    fn dispatch_sip() {
        let uri = Uri::parse("sip:alice@example.com").unwrap();
        assert!(uri
            .as_sip()
            .is_some());
        assert!(uri
            .as_tel()
            .is_none());
        assert!(uri
            .as_urn()
            .is_none());
    }

    #[test]
    fn dispatch_sips() {
        let uri = Uri::parse("sips:bob@secure.example.com").unwrap();
        assert!(uri
            .as_sip()
            .is_some());
    }

    #[test]
    fn dispatch_tel() {
        let uri = Uri::parse("tel:+15551234567").unwrap();
        assert!(uri
            .as_tel()
            .is_some());
        assert!(uri
            .as_sip()
            .is_none());
    }

    #[test]
    fn dispatch_urn() {
        let uri = Uri::parse("urn:service:sos").unwrap();
        assert!(uri
            .as_urn()
            .is_some());
        assert!(uri
            .as_sip()
            .is_none());
        assert!(uri
            .as_tel()
            .is_none());
    }

    #[test]
    fn unknown_scheme_stored_as_other() {
        let uri = Uri::parse("http://example.com").unwrap();
        assert_eq!(uri.as_other(), Some("http://example.com"));
        assert_eq!(uri.scheme(), Some("http"));
        assert!(uri
            .as_sip()
            .is_none());
        assert!(uri
            .as_tel()
            .is_none());
        assert!(uri
            .as_urn()
            .is_none());
    }

    #[test]
    fn other_display_roundtrip() {
        let input = "https://example.com/photo.jpg";
        let uri = Uri::parse(input).unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn missing_scheme_is_other() {
        let parsed = Uri::parse_with_warnings("no-colon-here").unwrap();
        assert_eq!(
            parsed
                .value
                .as_other(),
            Some("no-colon-here")
        );
        assert_eq!(
            parsed
                .value
                .scheme(),
            None
        );
        assert_eq!(parsed.warnings[0].code, WarningCode::MissingScheme);
        assert!(Uri::parse("").is_err());
    }

    #[test]
    fn display_roundtrip() {
        let input = "sip:alice@example.com;transport=tcp";
        let uri = Uri::parse(input).unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn display_roundtrip_urn() {
        let input = "urn:service:sos";
        let uri = Uri::parse(input).unwrap();
        assert_eq!(uri.to_string(), input);
    }

    #[test]
    fn from_sip_uri() {
        let sip = SipUri::parse("sip:alice@example.com").unwrap();
        let uri: Uri = sip.into();
        assert!(uri
            .as_sip()
            .is_some());
    }

    #[test]
    fn from_urn_uri() {
        let urn = UrnUri::parse("urn:service:sos").unwrap();
        let uri: Uri = urn.into();
        assert!(uri
            .as_urn()
            .is_some());
    }

    #[test]
    fn user_sip() {
        let uri = Uri::parse("sip:alice@example.com").unwrap();
        assert_eq!(uri.user(), Some("alice"));
    }

    #[test]
    fn user_sip_no_user() {
        let uri = Uri::parse("sip:example.com").unwrap();
        assert_eq!(uri.user(), None);
    }

    #[test]
    fn user_tel() {
        let uri = Uri::parse("tel:+15551234567").unwrap();
        assert_eq!(uri.user(), Some("+15551234567"));
    }

    #[test]
    fn user_urn() {
        let uri = Uri::parse("urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6").unwrap();
        assert_eq!(uri.user(), None);
    }

    #[test]
    fn user_other() {
        let uri = Uri::parse("http://example.com").unwrap();
        assert_eq!(uri.user(), None);
    }
}
