use reqwest::Url;

pub(super) struct Config {
    pub traces: Option<String>,
    pub metrics: Option<String>,
    pub logs: Option<String>,
    pub warnings: Vec<Warning>,
}

pub(super) struct Warning {
    pub signal: &'static str,
    pub key: &'static str,
}

#[derive(Clone, Copy)]
enum Signal {
    Traces,
    Metrics,
    Logs,
}

impl Signal {
    const fn export_name(self) -> &'static str {
        match self {
            Self::Traces => "traces",
            Self::Metrics => "metrics",
            Self::Logs => "logs",
        }
    }

    const fn v1_suffix(self) -> &'static str {
        match self {
            Self::Traces => "traces",
            Self::Metrics => "metrics",
            Self::Logs => "logs",
        }
    }

    const fn exporter_key(self) -> &'static str {
        match self {
            Self::Traces => "OTEL_TRACES_EXPORTER",
            Self::Metrics => "OTEL_METRICS_EXPORTER",
            Self::Logs => "OTEL_LOGS_EXPORTER",
        }
    }

    const fn endpoint_key(self) -> &'static str {
        match self {
            Self::Traces => "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
            Self::Metrics => "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
            Self::Logs => "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
        }
    }

    const fn protocol_key(self) -> &'static str {
        match self {
            Self::Traces => "OTEL_EXPORTER_OTLP_TRACES_PROTOCOL",
            Self::Metrics => "OTEL_EXPORTER_OTLP_METRICS_PROTOCOL",
            Self::Logs => "OTEL_EXPORTER_OTLP_LOGS_PROTOCOL",
        }
    }
}

enum BoolParse {
    Unset,
    True,
    False,
    Invalid,
}

enum ExporterSetting {
    Otlp,
    None,
    Invalid,
}

const GENERIC_ENDPOINT_KEY: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
const GENERIC_PROTOCOL_KEY: &str = "OTEL_EXPORTER_OTLP_PROTOCOL";
const SDK_DISABLED_KEY: &str = "OTEL_SDK_DISABLED";
const SUPPORTED_PROTOCOL: &str = "http/protobuf";

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

fn parse_bool(value: Option<String>) -> BoolParse {
    match value.map(|value| value.trim().to_owned()) {
        None => BoolParse::Unset,
        Some(v) if v.eq_ignore_ascii_case("true") => BoolParse::True,
        Some(v) if v.eq_ignore_ascii_case("false") => BoolParse::False,
        Some(_) => BoolParse::Invalid,
    }
}

fn protocol_supported(raw: Option<String>) -> bool {
    match nonempty(raw) {
        None => true,
        Some(v) if v.eq_ignore_ascii_case(SUPPORTED_PROTOCOL) => true,
        Some(_) => false,
    }
}

fn parse_exporter(raw: Option<String>) -> ExporterSetting {
    match nonempty(raw) {
        None => ExporterSetting::Otlp,
        Some(v) if v.eq_ignore_ascii_case("otlp") => ExporterSetting::Otlp,
        Some(v) if v.eq_ignore_ascii_case("none") => ExporterSetting::None,
        Some(_) => ExporterSetting::Invalid,
    }
}

fn valid_http_url(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https") && url.host().is_some()
}

fn resolve_from_generic(base: &str, signal: Signal) -> Option<String> {
    if !valid_http_url(base) {
        return None;
    }
    let mut url = Url::parse(base).expect("validated");
    let path = url.path().trim_end_matches('/');
    let suffix = signal.v1_suffix();
    let new_path = if path.is_empty() {
        format!("/v1/{suffix}")
    } else {
        format!("{path}/v1/{suffix}")
    };
    url.set_path(&new_path);
    Some(url.to_string())
}

fn resolve_signal(
    signal: Signal,
    lookup: &impl Fn(&str) -> Option<String>,
    generic_endpoint: Option<&str>,
    warnings: &mut Vec<Warning>,
) -> Option<String> {
    let specific_protocol = nonempty(lookup(signal.protocol_key()));
    let generic_protocol = nonempty(lookup(GENERIC_PROTOCOL_KEY));
    let (protocol_ok, protocol_warn_key) = if specific_protocol.is_some() {
        (protocol_supported(specific_protocol), signal.protocol_key())
    } else {
        (protocol_supported(generic_protocol), GENERIC_PROTOCOL_KEY)
    };
    if !protocol_ok {
        warnings.push(Warning {
            signal: signal.export_name(),
            key: protocol_warn_key,
        });
        return None;
    }

    match parse_exporter(lookup(signal.exporter_key())) {
        ExporterSetting::None => return None,
        ExporterSetting::Invalid => {
            warnings.push(Warning {
                signal: signal.export_name(),
                key: signal.exporter_key(),
            });
            return None;
        }
        ExporterSetting::Otlp => {}
    }

    let endpoint_key = signal.endpoint_key();
    if let Some(specific) = nonempty(lookup(endpoint_key)) {
        if valid_http_url(&specific) {
            return Some(specific);
        }
        warnings.push(Warning {
            signal: signal.export_name(),
            key: endpoint_key,
        });
        return None;
    }

    let base = generic_endpoint?;

    match resolve_from_generic(base, signal) {
        Some(url) => Some(url),
        None => {
            warnings.push(Warning {
                signal: signal.export_name(),
                key: GENERIC_ENDPOINT_KEY,
            });
            None
        }
    }
}

impl Config {
    pub fn read(lookup: impl Fn(&str) -> Option<String>) -> Self {
        match parse_bool(lookup(SDK_DISABLED_KEY)) {
            BoolParse::True => {
                return Self {
                    traces: None,
                    metrics: None,
                    logs: None,
                    warnings: Vec::new(),
                };
            }
            BoolParse::Invalid => {
                return Self {
                    traces: None,
                    metrics: None,
                    logs: None,
                    warnings: vec![Warning {
                        signal: "all",
                        key: SDK_DISABLED_KEY,
                    }],
                };
            }
            BoolParse::Unset | BoolParse::False => {}
        }

        let mut warnings = Vec::new();
        let generic_endpoint = nonempty(lookup(GENERIC_ENDPOINT_KEY));

        let traces = resolve_signal(
            Signal::Traces,
            &lookup,
            generic_endpoint.as_deref(),
            &mut warnings,
        );
        let metrics = resolve_signal(
            Signal::Metrics,
            &lookup,
            generic_endpoint.as_deref(),
            &mut warnings,
        );
        let logs = resolve_signal(
            Signal::Logs,
            &lookup,
            generic_endpoint.as_deref(),
            &mut warnings,
        );

        Self {
            traces,
            metrics,
            logs,
            warnings,
        }
    }
}
