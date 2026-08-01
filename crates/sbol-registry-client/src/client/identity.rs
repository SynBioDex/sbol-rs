//! Registry authentication and SBOL Identity OAuth flows.

use super::RegistryClient;
use crate::url::oauth_url;
use crate::{
    AuthorizationServerMetadata, OAuthClientRegistration, OAuthTokenResponse, RegistryError,
};

impl RegistryClient {
    /// Exchange registry credentials for a bearer token through the
    /// SynBioHub-compatible login endpoint.
    ///
    /// This is the compatibility fallback for registries that do not yet
    /// advertise an OAuth authorization issuer. Clients should prefer browser
    /// authorization when `machine_access.authorization_issuer` is present.
    pub fn password_login(
        &self,
        identifier: &str,
        password: &str,
    ) -> Result<String, RegistryError> {
        let url = self.endpoint("login")?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("email", identifier)
            .append_pair("password", password)
            .finish();
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "text/plain")
            .content_type("application/x-www-form-urlencoded")
            .send(body.as_bytes())
            .map_err(RegistryError::Transport)?;
        let response = self.buffer_response(response)?;
        let token = String::from_utf8(response.body)
            .map_err(RegistryError::InvalidUtf8)?
            .trim()
            .to_owned();
        if token.is_empty() {
            return Err(RegistryError::EmptyAccessToken);
        }
        Ok(token)
    }

    /// Discover the OAuth authorization server advertised by an SBOL DB
    /// instance. The issuer is supplied by `/api/v2/instance`; this method
    /// verifies that the returned metadata repeats it exactly.
    pub fn authorization_server(
        &self,
        issuer: &str,
    ) -> Result<AuthorizationServerMetadata, RegistryError> {
        let issuer_url = oauth_url(issuer, "OAuth issuer")?;
        let metadata_url = issuer_url
            .join(".well-known/oauth-authorization-server")
            .map_err(RegistryError::InvalidEndpoint)?;
        let response = self.get(metadata_url, "application/json")?;
        let metadata: AuthorizationServerMetadata =
            serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)?;
        if metadata.issuer != issuer {
            return Err(RegistryError::OAuthIssuerMismatch {
                expected: issuer.to_owned(),
                actual: metadata.issuer,
            });
        }
        if !metadata
            .code_challenge_methods_supported
            .iter()
            .any(|method| method == "S256")
        {
            return Err(RegistryError::OAuthCapability(
                "authorization server does not support S256 PKCE".to_owned(),
            ));
        }
        if !metadata
            .token_endpoint_auth_methods_supported
            .iter()
            .any(|method| method == "none")
        {
            return Err(RegistryError::OAuthCapability(
                "authorization server does not support public clients".to_owned(),
            ));
        }
        Ok(metadata)
    }

    /// Dynamically register a public authorization-code client for one exact
    /// loopback or HTTPS redirect URI.
    pub fn register_public_oauth_client(
        &self,
        metadata: &AuthorizationServerMetadata,
        client_name: &str,
        redirect_uri: &str,
    ) -> Result<OAuthClientRegistration, RegistryError> {
        let url = oauth_url(&metadata.registration_endpoint, "registration endpoint")?;
        let body = serde_json::to_vec(&serde_json::json!({
            "client_name": client_name,
            "redirect_uris": [redirect_uri],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none"
        }))
        .map_err(RegistryError::SerializeJson)?;
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "application/json")
            .content_type("application/json")
            .send(body.as_slice())
            .map_err(RegistryError::Transport)?;
        let response = self.buffer_response(response)?;
        serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)
    }

    /// Exchange a single-use authorization code using the original S256 PKCE
    /// verifier and exact protected-resource audience.
    pub fn exchange_oauth_code(
        &self,
        metadata: &AuthorizationServerMetadata,
        client_id: &str,
        redirect_uri: &str,
        resource: &str,
        code: &str,
        code_verifier: &str,
    ) -> Result<OAuthTokenResponse, RegistryError> {
        self.oauth_token_request(
            metadata,
            &[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", client_id),
                ("redirect_uri", redirect_uri),
                ("resource", resource),
                ("code_verifier", code_verifier),
            ],
        )
    }

    /// Rotate a saved OAuth refresh token. The original client and audience
    /// bindings are repeated so a credential cannot migrate across registries.
    pub fn refresh_oauth_token(
        &self,
        metadata: &AuthorizationServerMetadata,
        client_id: &str,
        resource: &str,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse, RegistryError> {
        self.oauth_token_request(
            metadata,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", client_id),
                ("resource", resource),
            ],
        )
    }

    /// Revoke an OAuth access or refresh token. The endpoint intentionally
    /// returns no token-validity signal.
    pub fn revoke_oauth_token(
        &self,
        metadata: &AuthorizationServerMetadata,
        token: &str,
    ) -> Result<(), RegistryError> {
        let endpoint = metadata.revocation_endpoint.as_deref().ok_or_else(|| {
            RegistryError::OAuthCapability(
                "authorization server does not advertise token revocation".to_owned(),
            )
        })?;
        let url = oauth_url(endpoint, "revocation endpoint")?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("token", token)
            .finish();
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "application/json")
            .content_type("application/x-www-form-urlencoded")
            .send(body.as_bytes())
            .map_err(RegistryError::Transport)?;
        self.buffer_response(response)?;
        Ok(())
    }

    /// Revoke a first-party compatibility token through the V2 session
    /// lifecycle. New SBOL Identity credentials use [`Self::revoke_oauth_token`].
    pub fn logout_compatibility_session(&self) -> Result<(), RegistryError> {
        let url = self.endpoint("api/v2/session")?;
        let mut request = self
            .agent
            .delete(url.as_str())
            .header("Accept", "application/json");
        if let Some(token) = &self.bearer_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        let response = request.call().map_err(RegistryError::Transport)?;
        self.buffer_response(response)?;
        Ok(())
    }

    fn oauth_token_request(
        &self,
        metadata: &AuthorizationServerMetadata,
        fields: &[(&str, &str)],
    ) -> Result<OAuthTokenResponse, RegistryError> {
        let url = oauth_url(&metadata.token_endpoint, "token endpoint")?;
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in fields {
            form.append_pair(name, value);
        }
        let body = form.finish();
        let response = self
            .agent
            .post(url.as_str())
            .header("Accept", "application/json")
            .content_type("application/x-www-form-urlencoded")
            .send(body.as_bytes())
            .map_err(RegistryError::Transport)?;
        let response = self.buffer_response(response)?;
        let tokens: OAuthTokenResponse =
            serde_json::from_slice(&response.body).map_err(RegistryError::InvalidJson)?;
        if !tokens.token_type.eq_ignore_ascii_case("bearer") {
            return Err(RegistryError::OAuthCapability(
                "token endpoint returned a non-Bearer access token".to_owned(),
            ));
        }
        Ok(tokens)
    }
}
