use crate::parse::{
    canonize_header, canonize_param, canonize_user, is_hnv_char, is_param_strict, is_paramchar,
    is_tel_paramchar, is_tel_pname_char, is_user_char, validate_pct_encoded,
};
use crate::warning::{Component, WarningCode, Warnings};

/// Character sets and canonization for one kind of `;`-separated param list.
pub(crate) struct ParamGrammar {
    component: Component,
    context: &'static str,
    /// Accepted; anything else is a parse error.
    allowed: fn(u8) -> bool,
    /// Conformant names and values; accepted bytes outside these warn.
    strict_name: fn(u8) -> bool,
    strict_value: fn(u8) -> bool,
    canonize: fn(&str) -> String,
}

/// SIP URI params after the host.
pub(crate) const SIP_PARAMS: ParamGrammar = ParamGrammar {
    component: Component::Param,
    context: "parameter",
    allowed: is_paramchar,
    strict_name: is_param_strict,
    strict_value: is_param_strict,
    canonize: canonize_param,
};

/// SIP user-params inside the userinfo.
pub(crate) const USER_PARAMS: ParamGrammar = ParamGrammar {
    component: Component::UserParam,
    context: "user parameter",
    allowed: is_user_char,
    strict_name: is_user_char,
    strict_value: is_user_char,
    canonize: canonize_user,
};

/// tel: URI params (RFC 3966).
pub(crate) const TEL_PARAMS: ParamGrammar = ParamGrammar {
    component: Component::Param,
    context: "parameter",
    allowed: is_paramchar,
    strict_name: is_tel_pname_char,
    strict_value: is_tel_paramchar,
    canonize: canonize_param,
};

/// Parse the params following a `;`, which is not included in `s`.
pub(crate) fn parse_params(
    s: &str,
    grammar: &ParamGrammar,
    warnings: &mut Warnings,
) -> Result<Vec<(String, Option<String>)>, String> {
    let component = grammar.component;
    let validate = |part: &str, what: &str| {
        validate_pct_encoded(part, grammar.allowed).map_err(|pos| {
            format!(
                "invalid character in {} {what} at position {pos}",
                grammar.context
            )
        })
    };

    let mut params = Vec::new();
    for part in s.split(';') {
        if part.is_empty() {
            warnings.push(component, WarningCode::EmptySegment, part, 0);
            continue;
        }
        let (name, value) = match part.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (part, None),
        };
        validate(name, "name")?;
        if name.is_empty() {
            warnings.push(component, WarningCode::EmptyName, part, 0);
        }
        warnings.charset(component, name, grammar.strict_name);
        if let Some(value) = value {
            validate(value, "value")?;
            warnings.charset(component, value, grammar.strict_value);
        }
        params.push((
            (grammar.canonize)(name),
            value.map(|v| (grammar.canonize)(v)),
        ));
    }
    Ok(params)
}

/// Parse header parameters following a `?`: `name=value` pairs separated by `&`.
pub(crate) fn parse_headers(
    s: &str,
    warnings: &mut Warnings,
) -> Result<Vec<(String, String)>, String> {
    let mut headers = Vec::new();
    for part in s.split('&') {
        if part.is_empty() {
            warnings.push(Component::Header, WarningCode::EmptySegment, part, 0);
            continue;
        }
        let Some((name, value)) = part.split_once('=') else {
            return Err("header missing '='".into());
        };
        if name.is_empty() {
            return Err("empty header name".into());
        }
        warnings.charset(Component::Header, name, is_hnv_char);
        warnings.charset(Component::Header, value, is_hnv_char);
        headers.push((canonize_header(name), canonize_header(value)));
    }
    Ok(headers)
}

/// Format parameters as a `;`-separated string with leading `;` for each.
pub(crate) fn format_params(
    params: &[(String, Option<String>)],
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    for (name, value) in params {
        write!(f, ";{name}")?;
        if let Some(v) = value {
            write!(f, "={v}")?;
        }
    }
    Ok(())
}

/// Format headers as `?name=value&name=value`.
pub(crate) fn format_headers(
    headers: &[(String, String)],
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    for (i, (name, value)) in headers
        .iter()
        .enumerate()
    {
        if i == 0 {
            write!(f, "?{name}={value}")?;
        } else {
            write!(f, "&{name}={value}")?;
        }
    }
    Ok(())
}

/// Look up a parameter by name (case-insensitive).
pub(crate) fn find_param<'a>(
    params: &'a [(String, Option<String>)],
    name: &str,
) -> Option<&'a Option<String>> {
    params
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sip_params(s: &str) -> Vec<(String, Option<String>)> {
        let mut w = Warnings::new(s);
        let params = parse_params(s, &SIP_PARAMS, &mut w).unwrap();
        assert!(!w
            .finish(())
            .has_warnings());
        params
    }

    #[test]
    fn parse_key_value() {
        let params = sip_params("transport=tcp");
        assert_eq!(params, vec![("transport".into(), Some("tcp".into()))]);
    }

    #[test]
    fn parse_mixed() {
        let params = sip_params("user=phone;ttl=1;isfocus");
        assert_eq!(
            params,
            vec![
                ("user".into(), Some("phone".into())),
                ("ttl".into(), Some("1".into())),
                ("isfocus".into(), None),
            ]
        );
    }

    #[test]
    fn parse_headers_basic() {
        let s = "From=foo@bar&To=bar@baz";
        let headers = parse_headers(s, &mut Warnings::new(s)).unwrap();
        assert_eq!(
            headers,
            vec![
                ("From".into(), "foo@bar".into()),
                ("To".into(), "bar@baz".into()),
            ]
        );
    }

    #[test]
    fn find_param_case_insensitive() {
        let params = vec![
            ("Transport".into(), Some("tcp".into())),
            ("user".into(), Some("phone".into())),
        ];
        assert_eq!(find_param(&params, "transport"), Some(&Some("tcp".into())));
        assert_eq!(find_param(&params, "USER"), Some(&Some("phone".into())));
        assert_eq!(find_param(&params, "missing"), None);
    }
}
