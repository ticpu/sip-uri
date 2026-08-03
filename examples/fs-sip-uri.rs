//! Parse a SIP/tel/URN URI for a FreeSWITCH dialplan.
//!
//! Invoked through FreeSWITCH's `spawn_stream` API, which runs the command via
//! `posix_spawnp` with no shell, so URI text coming from a SIP header cannot
//! reach a shell interpreter.
//!
//! ```xml
//! <condition field="${spawn_stream($${conf_dir}/bin/fs-sip-uri get user ${sip_h_X-Caller-Id-Number})}"
//!            expression="^$" break="on-true">
//!     <!-- endpoint URI with no user-part -->
//! </condition>
//! ```
//!
//! See `examples/freeswitch/README.md` for deployment and the full field list.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sip_uri::{SipUri, Uri};

/// Separator for the `vars` payload, matching the `^^|` prefix the dialplan
/// hands to `multiset`. Any component containing it aborts the whole payload:
/// `canonize_user` decodes `%3B`/`%3D` and never rejects a raw delimiter, so no
/// character is safe by construction.
const DELIM: char = '|';

/// Field lookup failed in a way the dialplan author must fix, as opposed to a
/// field that is simply absent from this URI.
const EXIT_BAD_FIELD: u8 = 2;

#[derive(Parser)]
#[command(
    name = "fs-sip-uri",
    version = concat!(
        "(sip-uri ",
        env!("CARGO_PKG_VERSION"),
        ") ",
        env!("CARGO_PKG_REPOSITORY")
    ),
    about = "Parse a SIP/tel/URN URI for a FreeSWITCH dialplan",
    after_help = concat!("Part of the sip-uri crate: ", env!("CARGO_PKG_REPOSITORY"))
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print one field, or nothing when the URI has no such component.
    Get {
        /// scheme, type, uri, user, password, host, port, nid, nss,
        /// param.<name>, uparam.<name>, header.<name>
        field: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        uri: Vec<String>,
    },
    /// Print every present component as a `multiset` payload.
    Vars {
        /// Prefix applied to every emitted variable name.
        prefix: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        uri: Vec<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (Cmd::Get { uri: raw, .. } | Cmd::Vars { uri: raw, .. }) = &cli.cmd;

    // FreeSWITCH splits the spawn command on blanks, so a header value carrying
    // a space arrives as several argv entries.
    let joined = raw.join(" ");
    let text = strip_angle_brackets(joined.trim());

    let uri: Uri = match text.parse() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("fs-sip-uri: cannot parse {text:?}: {e}");
            return ExitCode::FAILURE;
        }
    };

    match &cli.cmd {
        Cmd::Get { field, .. } => match get(&uri, field) {
            Ok(Some(value)) => println!("{value}"),
            Ok(None) => {}
            Err(e) => {
                eprintln!("fs-sip-uri: {e}");
                return ExitCode::from(EXIT_BAD_FIELD);
            }
        },
        Cmd::Vars { prefix, .. } => {
            let pairs = vars(&uri);
            if let Some((name, _)) = pairs
                .iter()
                .find(|(n, v)| n.contains(DELIM) || v.contains(DELIM))
            {
                eprintln!("fs-sip-uri: {name} contains {DELIM:?}, refusing to emit a payload");
                return ExitCode::FAILURE;
            }
            let payload: Vec<String> = pairs
                .iter()
                .map(|(name, value)| format!("{prefix}{name}={value}"))
                .collect();
            println!("{}", payload.join(&DELIM.to_string()));
        }
    }

    ExitCode::SUCCESS
}

/// Accept the `<sip:...>` wrapper a name-addr puts around the URI. A display
/// name or trailing header params are header grammar, not URI grammar, and are
/// left to fail in the parser.
fn strip_angle_brackets(s: &str) -> &str {
    s.strip_prefix('<')
        .and_then(|inner| inner.strip_suffix('>'))
        .unwrap_or(s)
}

fn uri_type(uri: &Uri) -> &'static str {
    match uri {
        Uri::Sip(_) => "sip",
        Uri::Tel(_) => "tel",
        Uri::Urn(_) => "urn",
        _ => "other",
    }
}

/// A parameter present without a value yields an empty string; FreeSWITCH
/// cannot distinguish that from an unset variable either way.
fn param_value(found: Option<&Option<String>>) -> Option<String> {
    found.map(|value| {
        value
            .clone()
            .unwrap_or_default()
    })
}

fn get(uri: &Uri, field: &str) -> Result<Option<String>, String> {
    if let Some((kind, name)) = field.split_once('.') {
        return match kind {
            "param" => Ok(match uri {
                Uri::Sip(u) => param_value(u.param(name)),
                Uri::Tel(u) => param_value(u.param(name)),
                _ => None,
            }),
            "uparam" => Ok(uri
                .as_sip()
                .and_then(|u| {
                    param_value(
                        u.user_params()
                            .iter()
                            .find(|(n, _)| n.eq_ignore_ascii_case(name))
                            .map(|(_, v)| v),
                    )
                })),
            "header" => Ok(uri
                .as_sip()
                .and_then(|u| u.header(name))
                .map(str::to_string)),
            _ => Err(format!("unknown field {field:?}")),
        };
    }

    Ok(match field {
        "type" => Some(uri_type(uri).to_string()),
        "scheme" => Some(
            uri.scheme()
                .to_string(),
        ),
        "uri" => Some(uri.to_string()),
        "user" => uri
            .user()
            .map(str::to_string),
        "password" => uri
            .as_sip()
            .and_then(SipUri::password)
            .map(str::to_string),
        "host" => uri
            .as_sip()
            .map(|u| {
                u.host()
                    .to_string()
            }),
        "port" => uri
            .as_sip()
            .and_then(SipUri::port)
            .map(|p| p.to_string()),
        "nid" => uri
            .as_urn()
            .map(|u| {
                u.nid()
                    .to_string()
            }),
        "nss" => uri
            .as_urn()
            .map(|u| {
                u.nss()
                    .to_string()
            }),
        _ => return Err(format!("unknown field {field:?}")),
    })
}

fn vars(uri: &Uri) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |name: String, value: String| out.push((name, value));

    push("type".into(), uri_type(uri).into());
    push(
        "scheme".into(),
        uri.scheme()
            .into(),
    );
    if let Some(user) = uri.user() {
        push("user".into(), user.into());
    }

    match uri {
        Uri::Sip(u) => {
            push(
                "host".into(),
                u.host()
                    .to_string(),
            );
            if let Some(port) = u.port() {
                push("port".into(), port.to_string());
            }
            if let Some(password) = u.password() {
                push("password".into(), password.into());
            }
            for (name, value) in u.user_params() {
                push(
                    format!("uparam_{}", name.to_lowercase()),
                    value
                        .clone()
                        .unwrap_or_default(),
                );
            }
            for (name, value) in u.params() {
                push(
                    format!("param_{}", name.to_lowercase()),
                    value
                        .clone()
                        .unwrap_or_default(),
                );
            }
            for (name, value) in u.headers() {
                push(format!("header_{}", name.to_lowercase()), value.clone());
            }
        }
        Uri::Tel(u) => {
            for (name, value) in u.params() {
                push(
                    format!("param_{}", name.to_lowercase()),
                    value
                        .clone()
                        .unwrap_or_default(),
                );
            }
        }
        Uri::Urn(u) => {
            push(
                "nid".into(),
                u.nid()
                    .into(),
            );
            push(
                "nss".into(),
                u.nss()
                    .into(),
            );
        }
        _ => {}
    }

    out
}
