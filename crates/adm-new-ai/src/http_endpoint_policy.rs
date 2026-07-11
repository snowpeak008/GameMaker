use std::fmt;
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HttpEndpointPolicyError {
    InvalidUrl,
    UnsupportedScheme,
    MissingHost,
    RemoteHttp,
}

impl fmt::Display for HttpEndpointPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl => formatter.write_str("is not a valid URL"),
            Self::UnsupportedScheme => formatter.write_str(
                "must use HTTPS, or HTTP for localhost, 127.0.0.0/8, or ::1",
            ),
            Self::MissingHost => formatter.write_str("has no host"),
            Self::RemoteHttp => formatter.write_str(
                "must use HTTPS for remote hosts; HTTP is allowed only for localhost, 127.0.0.0/8, or ::1",
            ),
        }
    }
}

pub(crate) fn validate_http_transport_url(
    value: &str,
) -> Result<reqwest::Url, HttpEndpointPolicyError> {
    let url = reqwest::Url::parse(value.trim()).map_err(|_| HttpEndpointPolicyError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(HttpEndpointPolicyError::UnsupportedScheme);
    }
    if url.host_str().is_none() {
        return Err(HttpEndpointPolicyError::MissingHost);
    }
    if url.scheme() == "http" && !is_loopback_host(&url) {
        return Err(HttpEndpointPolicyError::RemoteHttp);
    }
    Ok(url)
}

fn is_loopback_host(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_is_allowed_for_remote_and_private_hosts() {
        for url in [
            "https://api.example.test/v1",
            "https://192.168.1.20/v1",
            "https://10.0.0.8/v1",
            "https://[2001:db8::1]/v1",
        ] {
            assert!(validate_http_transport_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn http_is_allowed_only_for_explicit_loopback_hosts() {
        for url in [
            "http://localhost:11434/v1",
            "http://127.0.0.0/v1",
            "http://127.23.45.67/v1",
            "http://127.255.255.255/v1",
            "http://[::1]:11434/v1",
        ] {
            assert!(validate_http_transport_url(url).is_ok(), "{url}");
        }

        for url in [
            "http://api.example.test/v1",
            "http://localhost.example.test/v1",
            "http://10.0.0.8/v1",
            "http://192.168.1.20/v1",
            "http://0.0.0.0/v1",
            "http://[::]/v1",
            "http://[::ffff:127.0.0.1]/v1",
        ] {
            assert_eq!(
                validate_http_transport_url(url),
                Err(HttpEndpointPolicyError::RemoteHttp),
                "{url}"
            );
        }
    }

    #[test]
    fn non_http_schemes_and_invalid_urls_are_rejected() {
        assert_eq!(
            validate_http_transport_url("ftp://localhost/file"),
            Err(HttpEndpointPolicyError::UnsupportedScheme)
        );
        assert_eq!(
            validate_http_transport_url("not a URL"),
            Err(HttpEndpointPolicyError::InvalidUrl)
        );
    }
}
