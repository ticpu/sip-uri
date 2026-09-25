use crate::grammar::{
    is_hnv_char, is_param_strict, is_tel_paramchar, is_tel_pname_char, is_user_char,
};
use crate::warning::{Component, WarningCode, Warnings};

pub(crate) type Params = Vec<(String, Option<String>)>;

/// Character sets for one kind of `;`-separated param list.
pub(crate) struct ParamGrammar {
    component: Component,
    /// Conformant names and values; any other byte warns.
    name: fn(u8) -> bool,
    value: fn(u8) -> bool,
}

/// SIP URI params after the host.
pub(crate) const SIP_PARAMS: ParamGrammar = ParamGrammar {
    component: Component::Param,
    name: is_param_strict,
    value: is_param_strict,
};

/// SIP user-params inside the userinfo.
pub(crate) const USER_PARAMS: ParamGrammar = ParamGrammar {
    component: Component::UserParam,
    name: is_user_char,
    value: is_user_char,
};

/// tel: URI params (RFC 3966).
pub(crate) const TEL_PARAMS: ParamGrammar = ParamGrammar {
    component: Component::Param,
    name: is_tel_pname_char,
    value: is_tel_paramchar,
};

/// Split the params following a `;`, which is not included in `s`.
pub(crate) fn parse_params(s: &str, grammar: &ParamGrammar, warnings: &mut Warnings) -> Params {
    let component = grammar.component;
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
        if name.is_empty() {
            warnings.push(component, WarningCode::EmptyName, part, 0);
        }
        warnings.charset(component, name, grammar.name);
        if let Some(value) = value {
            warnings.charset(component, value, grammar.value);
        }
        params.push((name.to_string(), value.map(str::to_string)));
    }
    params
}

/// Split header parameters following a `?`: `name=value` pairs separated by `&`.
pub(crate) fn parse_headers(s: &str, warnings: &mut Warnings) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    for part in s.split('&') {
        if part.is_empty() {
            warnings.push(Component::Header, WarningCode::EmptySegment, part, 0);
            continue;
        }
        let (name, value) = part
            .split_once('=')
            .unwrap_or_else(|| {
                warnings.push(
                    Component::Header,
                    WarningCode::MissingValue,
                    part,
                    part.len(),
                );
                (part, "")
            });
        if name.is_empty() {
            warnings.push(Component::Header, WarningCode::EmptyName, part, 0);
        }
        warnings.charset(Component::Header, name, is_hnv_char);
        warnings.charset(Component::Header, value, is_hnv_char);
        headers.push((name.to_string(), value.to_string()));
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sip_params(s: &str) -> Params {
        let mut w = Warnings::new(s);
        let params = parse_params(s, &SIP_PARAMS, &mut w);
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
        let headers = parse_headers(s, &mut Warnings::new(s));
        assert_eq!(
            headers,
            vec![
                ("From".into(), "foo@bar".into()),
                ("To".into(), "bar@baz".into()),
            ]
        );
    }
}
