use crate::parse::{canonize_param, canonize_user, is_paramchar, is_user_char};

/// Parse a `;`-separated parameter string into a list of `(name, Option<value>)` pairs.
///
/// Input should NOT include the leading `;`. Parameters are separated by `;`.
/// Each parameter is `name` or `name=value`. Values and names are canonized
/// (percent-decoded where allowed by the paramchar set).
pub(crate) fn parse_params(s: &str) -> Result<Vec<(String, Option<String>)>, String> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut params = Vec::new();

    for part in s.split(';') {
        if part.is_empty() {
            continue;
        }

        if let Some((name, value)) = part.split_once('=') {
            validate_param_chars(name, "parameter name")?;
            validate_param_chars(value, "parameter value")?;
            params.push((canonize_param(name), Some(canonize_param(value))));
        } else {
            validate_param_chars(part, "parameter name")?;
            params.push((canonize_param(part), None));
        }
    }

    Ok(params)
}

/// Parse user-params from the userinfo section of a SIP URI.
///
/// User-params use the `user` character set (unreserved + user-unreserved)
/// which includes `?` and `/`, unlike URI params which use `paramchar`.
pub(crate) fn parse_user_params(s: &str) -> Result<Vec<(String, Option<String>)>, String> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut params = Vec::new();

    for part in s.split(';') {
        if part.is_empty() {
            continue;
        }

        if let Some((name, value)) = part.split_once('=') {
            validate_user_param_chars(name, "user parameter name")?;
            validate_user_param_chars(value, "user parameter value")?;
            params.push((canonize_user(name), Some(canonize_user(value))));
        } else {
            validate_user_param_chars(part, "user parameter name")?;
            params.push((canonize_user(part), None));
        }
    }

    Ok(params)
}

fn validate_user_param_chars(s: &str, context: &str) -> Result<(), String> {
    crate::parse::validate_pct_encoded(s, is_user_char)
        .map_err(|pos| format!("invalid character in {context} at position {pos}"))
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
            headers.push((
                crate::parse::canonize_header(name),
                crate::parse::canonize_header(value),
            ));
        } else {
            return Err(format!("header missing '=' in '{part}'"));
        }
    }

    Ok(headers)
}

fn validate_param_chars(s: &str, context: &str) -> Result<(), String> {
    crate::parse::validate_pct_encoded(s, is_paramchar)
        .map_err(|pos| format!("invalid character in {context} at position {pos}"))
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
