pub(crate) type Params = Vec<(String, Option<String>)>;

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

type Canonize = fn(&str, Option<&str>) -> (String, Option<String>);

/// Push the canonized pair, unless it is a valueless empty name: its bare `;`
/// parses as no param.
pub(crate) fn push_pair(params: &mut Params, name: &str, value: Option<&str>, canonize: Canonize) {
    if !name.is_empty() || value.is_some() {
        params.push(canonize(name, value));
    }
}

/// Canonize every pair with `canonize`, as [`push_pair`] does.
pub(crate) fn canonize_pairs(params: Params, canonize: Canonize) -> Params {
    let mut out = Vec::with_capacity(params.len());
    for (name, value) in &params {
        push_pair(&mut out, name, value.as_deref(), canonize);
    }
    out
}

/// Look up a parameter by name (case-insensitive).
pub(crate) fn find_param<'a>(
    params: &'a [(String, Option<String>)],
    name: &str,
) -> Option<Option<&'a str>> {
    params
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_param_case_insensitive() {
        let params = vec![
            ("Transport".into(), Some("tcp".into())),
            ("user".into(), Some("phone".into())),
        ];
        assert_eq!(find_param(&params, "transport"), Some(Some("tcp")));
        assert_eq!(find_param(&params, "USER"), Some(Some("phone")));
        assert_eq!(find_param(&params, "missing"), None);
    }
}
