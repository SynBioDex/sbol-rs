//! URL normalization and transport-boundary validation.

use url::{Host, Url};

use crate::RegistryError;

pub(crate) fn oauth_url(value: &str, kind: &'static str) -> Result<Url, RegistryError> {
    let url = Url::parse(value).map_err(|_| RegistryError::InvalidOAuthUrl {
        kind,
        url: value.to_owned(),
    })?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || !is_secure_registry_url(&url)
    {
        return Err(RegistryError::InvalidOAuthUrl {
            kind,
            url: value.to_owned(),
        });
    }
    Ok(url)
}

pub(crate) fn normalize_registry_url(value: &str) -> Result<Url, RegistryError> {
    let mut url = Url::parse(value).map_err(|source| RegistryError::InvalidRegistryUrl {
        url: value.to_owned(),
        source,
    })?;
    if !is_secure_registry_url(&url) {
        return Err(RegistryError::UnsupportedRegistryUrl(value.to_owned()));
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(RegistryError::InvalidRegistryBase(value.to_owned()));
    }
    let path = format!("{}/", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

pub(crate) fn validate_design_iri(value: &str) -> Result<(), RegistryError> {
    let url = Url::parse(value).map_err(|source| RegistryError::InvalidDesignIri {
        iri: value.to_owned(),
        source,
    })?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(RegistryError::CredentialedDesignIri(value.to_owned()));
    }
    Ok(())
}

/// Registry credentials and uploaded biological data must not cross a cleartext
/// network boundary. Plain HTTP transport is accepted only for an actual
/// loopback host so local development can use an ephemeral server without
/// weakening production defaults. Design IRIs are identifiers sent to this
/// already-validated registry endpoint, not transport destinations; legacy
/// `http://` identities therefore remain usable with an explicit registry.
pub(crate) fn is_secure_registry_url(url: &Url) -> bool {
    match url.scheme() {
        "https" => url.host().is_some(),
        "http" => match url.host() {
            Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
            Some(Host::Ipv4(address)) => address.is_loopback(),
            Some(Host::Ipv6(address)) => address.is_loopback(),
            None => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_secret_bearing_registry_urls_and_design_iris() {
        assert!(matches!(
            normalize_registry_url("https://alice:secret@example.org"),
            Err(RegistryError::InvalidRegistryBase(_))
        ));
        assert!(matches!(
            normalize_registry_url("https://example.org?token=secret"),
            Err(RegistryError::InvalidRegistryBase(_))
        ));
        assert!(matches!(
            validate_design_iri("https://alice:secret@example.org/design/1"),
            Err(RegistryError::CredentialedDesignIri(_))
        ));
    }

    #[test]
    fn accepts_http_only_for_actual_loopback_hosts() {
        for value in [
            "http://localhost:8888",
            "http://127.0.0.1:8888",
            "http://127.25.4.3:8888",
            "http://[::1]:8888",
        ] {
            normalize_registry_url(value).unwrap_or_else(|error| panic!("{value}: {error}"));
        }

        for value in [
            "http://example.org",
            "http://localhost.example.org",
            "http://192.168.1.10:8888",
        ] {
            assert!(matches!(
                normalize_registry_url(value),
                Err(RegistryError::UnsupportedRegistryUrl(_))
            ));
        }
    }

    #[test]
    fn explicit_registry_accepts_legacy_http_design_identifiers() {
        validate_design_iri("http://synbiohub.org/public/igem/BBa_J23100/1").unwrap();
    }
}
