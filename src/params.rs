use crate::parse::{
    canonize_header, canonize_param, canonize_user, is_paramchar, is_user_char,
    validate_pct_encoded,
};

/// Parse a `;`-separated parameter string into a list of `(name, Option<value>)` pairs.
///
/// Input should NOT include the leading `;`. Each parameter is `name` or
/// `name=value`, validated against and canonized for the paramchar set.
pub(crate) fn parse_params(s: &str) -> Result<Vec<(String, Option<String>)>, String> {
    parse_param_list(s, is_paramchar, canonize_param, "parameter")
}

/// Parse user-params from the userinfo section of a SIP URI.
///
/// User-params use the `user` character set (unreserved + user-unreserved)
/// which includes `?` and `/`, unlike URI params which use `paramchar`.
pub(crate) fn parse_user_params(s: &str) -> Result<Vec<(String, Option<String>)>, String> {
    parse_param_list(s, is_user_char, canonize_user, "user parameter")
}

fn parse_param_list(
    s: &str,
    allowed: fn(u8) -> bool,
    canonize: fn(&str) -> String,
    context: &str,
) -> Result<Vec<(String, Option<String>)>, String> {
    let validate = |part: &str, what: &str| {
        validate_pct_encoded(part, allowed)
            .map_err(|pos| format!("invalid character in {context} {what} at position {pos}"))
    };

    let mut params = Vec::new();
    for part in s.split(';') {
        if part.is_empty() {
            continue;
        }
        if let Some((name, value)) = part.split_once('=') {
            validate(name, "name")?;
            validate(value, "value")?;
            params.push((canonize(name), Some(canonize(value))));
        } else {
            validate(part, "name")?;
            params.push((canonize(part), None));
        }
    }
    Ok(params)
}

/// Parse header parameters from the `?` section: `name=value` pairs separated by `&`.
pub(crate) fn parse_headers(s: &str) -> Result<Vec<(String, String)>, String> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut headers = Vec::new();

    for part in s.split('&') {
        if part.is_empty() {
            continue;
        }

        // RFC 3261 §25: header = hname "=" hvalue
        // hvalue can be empty
        if let Some((name, value)) = part.split_once('=') {
            if name.is_empty() {
                return Err("empty header name".into());
            }
            headers.push((canonize_header(name), canonize_header(value)));
        } else {
            return Err("header missing '='".into());
        }
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

    #[test]
    fn parse_empty() {
        assert_eq!(parse_params("").unwrap(), vec![]);
    }

    #[test]
    fn parse_key_value() {
        let params = parse_params("transport=tcp").unwrap();
        assert_eq!(params, vec![("transport".into(), Some("tcp".into()))]);
    }

    #[test]
    fn parse_mixed() {
        let params = parse_params("user=phone;ttl=1;isfocus").unwrap();
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
        let headers = parse_headers("From=foo@bar&To=bar@baz").unwrap();
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
