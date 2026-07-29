//! Pure BUD-11 authorization claim construction, parsing, and validation.
//!
//! This module validates bounded claim content and caller-supplied endpoint,
//! server, hash, and time context. It does not read a clock, verify Nostr event
//! signatures or authors, encode HTTP headers, send requests, or publish events.
//! A [`ValidatedAuthorizationClaim`] therefore proves only the checks represented
//! by its [`AuthorizationValidation`]; the composing Nostr and HTTP layers retain
//! authentication and request-commit responsibility.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{fmt, str::FromStr};

use crate::{Error, Sha256};

pub const RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND: u16 = 24_242;
pub const RADROOTS_BLOSSOM_AUTH_CONTENT_MAX_BYTES: usize = 4_096;
pub const RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS: u64 = 300;
pub const RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS: u64 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AuthorizationAction {
    Get,
    Upload,
    List,
    Delete,
    Media,
}

impl AuthorizationAction {
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "get" => Ok(Self::Get),
            "upload" => Ok(Self::Upload),
            "list" => Ok(Self::List),
            "delete" => Ok(Self::Delete),
            "media" => Ok(Self::Media),
            _ => Err(Error::InvalidAuthorizationAction),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Upload => "upload",
            Self::List => "list",
            Self::Delete => "delete",
            Self::Media => "media",
        }
    }
}

impl fmt::Display for AuthorizationAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AuthorizationAction {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ServerDomain(String);

impl ServerDomain {
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > 253
            || !value.is_ascii()
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(Error::InvalidAuthorizationServerDomain);
        }

        let mut all_labels_are_numeric = true;
        for label in value.split('.') {
            if label.is_empty()
                || label.len() > 63
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || !label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                || !label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
            {
                return Err(Error::InvalidAuthorizationServerDomain);
            }
            all_labels_are_numeric &= label.bytes().all(|byte| byte.is_ascii_digit());
        }

        if all_labels_are_numeric && !is_canonical_dotted_ipv4(value) {
            return Err(Error::InvalidAuthorizationServerDomain);
        }

        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServerDomain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ServerDomain {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AuthorizationContent(String);

impl AuthorizationContent {
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.is_empty()
            || value.len() > RADROOTS_BLOSSOM_AUTH_CONTENT_MAX_BYTES
            || value.trim() != value
            || value
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\t' | '\n' | '\r'))
        {
            return Err(Error::InvalidAuthorizationContent);
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AuthorizationContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AuthorizationContent {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AuthorizationTarget {
    GetBlob(Sha256),
    Upload(Sha256),
    List,
    DeleteBlob(Sha256),
    Mirror(Sha256),
    Media(Sha256),
}

impl AuthorizationTarget {
    pub const fn action(self) -> AuthorizationAction {
        match self {
            Self::GetBlob(_) => AuthorizationAction::Get,
            Self::Upload(_) | Self::Mirror(_) => AuthorizationAction::Upload,
            Self::List => AuthorizationAction::List,
            Self::DeleteBlob(_) => AuthorizationAction::Delete,
            Self::Media(_) => AuthorizationAction::Media,
        }
    }

    pub const fn implied_hash(self) -> Option<Sha256> {
        match self {
            Self::GetBlob(hash)
            | Self::Upload(hash)
            | Self::DeleteBlob(hash)
            | Self::Mirror(hash)
            | Self::Media(hash) => Some(hash),
            Self::List => None,
        }
    }

    const fn requires_hash_tag(self) -> bool {
        match self {
            Self::GetBlob(_) | Self::List => false,
            Self::Upload(_) | Self::DeleteBlob(_) | Self::Mirror(_) | Self::Media(_) => true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServerScopeRequirement {
    OptionalAnyMatch,
    RequiredAnyMatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationValidation {
    target: AuthorizationTarget,
    target_server: ServerDomain,
    server_scope_requirement: ServerScopeRequirement,
    now: u64,
    max_created_age_seconds: Option<u64>,
    max_lifetime_seconds: Option<u64>,
}

impl AuthorizationValidation {
    pub fn bud11(target: AuthorizationTarget, target_server: ServerDomain, now: u64) -> Self {
        Self {
            target,
            target_server,
            server_scope_requirement: ServerScopeRequirement::OptionalAnyMatch,
            now,
            max_created_age_seconds: None,
            max_lifetime_seconds: None,
        }
    }

    pub fn new(
        target: AuthorizationTarget,
        target_server: ServerDomain,
        server_scope_requirement: ServerScopeRequirement,
        now: u64,
        max_created_age_seconds: u64,
    ) -> Result<Self, Error> {
        if max_created_age_seconds > RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS {
            return Err(Error::InvalidAuthorizationCreatedAge);
        }
        Ok(Self {
            target,
            target_server,
            server_scope_requirement,
            now,
            max_created_age_seconds: Some(max_created_age_seconds),
            max_lifetime_seconds: Some(RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS),
        })
    }

    pub const fn target(&self) -> AuthorizationTarget {
        self.target
    }

    pub fn target_server(&self) -> &ServerDomain {
        &self.target_server
    }

    pub const fn server_scope_requirement(&self) -> ServerScopeRequirement {
        self.server_scope_requirement
    }

    pub const fn now(&self) -> u64 {
        self.now
    }

    pub const fn max_created_age_seconds(&self) -> Option<u64> {
        self.max_created_age_seconds
    }

    pub const fn max_lifetime_seconds(&self) -> Option<u64> {
        self.max_lifetime_seconds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationClaim {
    content: AuthorizationContent,
    created_at: u64,
    action: AuthorizationAction,
    expiration: u64,
    server_domains: Vec<ServerDomain>,
    hashes: Vec<Sha256>,
}

impl AuthorizationClaim {
    pub fn parse(content: &str, created_at: u64, tags: &[Vec<String>]) -> Result<Self, Error> {
        let content = AuthorizationContent::parse(content)?;
        let mut action = None;
        let mut expiration = None;
        let mut server_domains = Vec::new();
        let mut hashes = Vec::new();

        for tag in tags {
            match tag.first().map(String::as_str) {
                Some("t") => {
                    if action.is_some() {
                        return Err(Error::DuplicateAuthorizationActionTag);
                    }
                    let value = tag.get(1).ok_or(Error::MalformedAuthorizationActionTag)?;
                    action = Some(
                        AuthorizationAction::parse(value)
                            .map_err(|_| Error::MalformedAuthorizationActionTag)?,
                    );
                }
                Some("expiration") => {
                    if expiration.is_some() {
                        return Err(Error::DuplicateAuthorizationExpirationTag);
                    }
                    let value = tag
                        .get(1)
                        .ok_or(Error::MalformedAuthorizationExpirationTag)?;
                    expiration = Some(parse_timestamp(value)?);
                }
                Some("server") => {
                    let value = tag.get(1).ok_or(Error::MalformedAuthorizationServerTag)?;
                    server_domains.push(
                        ServerDomain::parse(value)
                            .map_err(|_| Error::MalformedAuthorizationServerTag)?,
                    );
                }
                Some("x") => {
                    let value = tag.get(1).ok_or(Error::MalformedAuthorizationHashTag)?;
                    hashes.push(
                        Sha256::from_hex(value)
                            .map_err(|_| Error::MalformedAuthorizationHashTag)?,
                    );
                }
                _ => {}
            }
        }

        Ok(Self {
            content,
            created_at,
            action: action.ok_or(Error::MissingAuthorizationActionTag)?,
            expiration: expiration.ok_or(Error::MissingAuthorizationExpirationTag)?,
            server_domains,
            hashes,
        })
    }

    pub fn validate(
        &self,
        validation: &AuthorizationValidation,
    ) -> Result<ValidatedAuthorizationClaim, Error> {
        if self.created_at >= validation.now {
            return Err(Error::AuthorizationCreatedInFuture);
        }
        if let Some(max_created_age_seconds) = validation.max_created_age_seconds
            && validation.now.saturating_sub(self.created_at) > max_created_age_seconds
        {
            return Err(Error::AuthorizationStale);
        }

        let lifetime = self
            .expiration
            .checked_sub(self.created_at)
            .filter(|lifetime| *lifetime > 0)
            .ok_or(Error::InvalidAuthorizationLifetime)?;
        if let Some(max_lifetime_seconds) = validation.max_lifetime_seconds
            && lifetime > max_lifetime_seconds
        {
            return Err(Error::InvalidAuthorizationLifetime);
        }

        if self.expiration <= validation.now {
            return Err(Error::AuthorizationExpired);
        }
        if self.action != validation.target.action() {
            return Err(Error::AuthorizationActionMismatch);
        }

        self.validate_server_scope(validation)?;
        self.validate_hash_scope(validation.target)?;

        Ok(ValidatedAuthorizationClaim {
            claim: self.clone(),
            target: validation.target,
            target_server: validation.target_server.clone(),
        })
    }

    pub fn content(&self) -> &AuthorizationContent {
        &self.content
    }

    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub const fn action(&self) -> AuthorizationAction {
        self.action
    }

    pub const fn expiration(&self) -> u64 {
        self.expiration
    }

    pub fn server_domains(&self) -> &[ServerDomain] {
        &self.server_domains
    }

    pub fn hashes(&self) -> &[Sha256] {
        &self.hashes
    }

    fn validate_server_scope(&self, validation: &AuthorizationValidation) -> Result<(), Error> {
        if self.server_domains.is_empty() {
            return match validation.server_scope_requirement {
                ServerScopeRequirement::OptionalAnyMatch => Ok(()),
                ServerScopeRequirement::RequiredAnyMatch => Err(Error::AuthorizationServerRequired),
            };
        }
        if self
            .server_domains
            .iter()
            .any(|server| server == &validation.target_server)
        {
            Ok(())
        } else {
            Err(Error::AuthorizationServerMismatch)
        }
    }

    fn validate_hash_scope(&self, target: AuthorizationTarget) -> Result<(), Error> {
        let Some(implied_hash) = target.implied_hash() else {
            return Ok(());
        };
        if self.hashes.is_empty() {
            return if target.requires_hash_tag() {
                Err(Error::AuthorizationHashRequired)
            } else {
                Ok(())
            };
        }
        if self.hashes.contains(&implied_hash) {
            Ok(())
        } else {
            Err(Error::AuthorizationHashMismatch)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedAuthorizationClaim {
    claim: AuthorizationClaim,
    target: AuthorizationTarget,
    target_server: ServerDomain,
}

impl ValidatedAuthorizationClaim {
    pub fn claim(&self) -> &AuthorizationClaim {
        &self.claim
    }

    pub const fn target(&self) -> AuthorizationTarget {
        self.target
    }

    pub fn target_server(&self) -> &ServerDomain {
        &self.target_server
    }

    pub fn content(&self) -> &AuthorizationContent {
        self.claim.content()
    }

    pub const fn created_at(&self) -> u64 {
        self.claim.created_at()
    }

    pub const fn action(&self) -> AuthorizationAction {
        self.claim.action()
    }

    pub const fn expiration(&self) -> u64 {
        self.claim.expiration()
    }

    pub fn server_domains(&self) -> &[ServerDomain] {
        self.claim.server_domains()
    }

    pub fn hashes(&self) -> &[Sha256] {
        self.claim.hashes()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationWireParts {
    kind: u16,
    created_at: u64,
    tags: Vec<Vec<String>>,
    content: String,
}

impl AuthorizationWireParts {
    pub const fn kind(&self) -> u16 {
        self.kind
    }

    pub const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn tags(&self) -> &[Vec<String>] {
        &self.tags
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn into_parts(self) -> (u16, u64, Vec<Vec<String>>, String) {
        (self.kind, self.created_at, self.tags, self.content)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredUploadClaim {
    wire_parts: AuthorizationWireParts,
    content: AuthorizationContent,
    server_domain: ServerDomain,
    sha256: Sha256,
    expiration: u64,
}

impl AuthoredUploadClaim {
    pub fn new(
        content: AuthorizationContent,
        server_domain: ServerDomain,
        sha256: Sha256,
        created_at: u64,
        lifetime_seconds: u64,
    ) -> Result<Self, Error> {
        if !(1..=RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS).contains(&lifetime_seconds) {
            return Err(Error::InvalidAuthorizationLifetime);
        }
        let expiration = created_at
            .checked_add(lifetime_seconds)
            .ok_or(Error::AuthorizationTimestampOverflow)?;
        let tags = vec![
            vec!["t".to_string(), "upload".to_string()],
            vec!["expiration".to_string(), expiration.to_string()],
            vec!["x".to_string(), sha256.to_hex()],
            vec!["server".to_string(), server_domain.as_str().to_string()],
        ];
        let wire_parts = AuthorizationWireParts {
            kind: RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND,
            created_at,
            tags,
            content: content.as_str().to_string(),
        };
        Ok(Self {
            wire_parts,
            content,
            server_domain,
            sha256,
            expiration,
        })
    }

    pub fn content(&self) -> &AuthorizationContent {
        &self.content
    }

    pub fn server_domain(&self) -> &ServerDomain {
        &self.server_domain
    }

    pub const fn sha256(&self) -> Sha256 {
        self.sha256
    }

    pub const fn created_at(&self) -> u64 {
        self.wire_parts.created_at
    }

    pub const fn expiration(&self) -> u64 {
        self.expiration
    }

    pub const fn lifetime_seconds(&self) -> u64 {
        self.expiration - self.wire_parts.created_at
    }

    pub fn wire_parts(&self) -> &AuthorizationWireParts {
        &self.wire_parts
    }

    pub fn into_wire_parts(self) -> AuthorizationWireParts {
        self.wire_parts
    }
}

fn parse_timestamp(value: &str) -> Result<u64, Error> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::MalformedAuthorizationExpirationTag);
    }
    value
        .parse()
        .map_err(|_| Error::MalformedAuthorizationExpirationTag)
}

fn is_canonical_dotted_ipv4(value: &str) -> bool {
    let mut count = 0_usize;
    for octet in value.split('.') {
        count += 1;
        if (octet.len() > 1 && octet.starts_with('0')) || octet.parse::<u8>().is_err() {
            return false;
        }
    }
    count == 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, vec};

    const NOW: u64 = 1_800_000_000;
    const HASH_HEX: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn hash() -> Sha256 {
        Sha256::from_hex(HASH_HEX).unwrap()
    }

    fn server() -> ServerDomain {
        ServerDomain::parse("media.example.com").unwrap()
    }

    fn content() -> AuthorizationContent {
        AuthorizationContent::parse("Upload Blob").unwrap()
    }

    fn tags(action: &str, expiration: u64) -> Vec<Vec<String>> {
        vec![
            vec!["t".to_string(), action.to_string()],
            vec!["expiration".to_string(), expiration.to_string()],
            vec!["x".to_string(), HASH_HEX.to_string()],
            vec!["server".to_string(), "media.example.com".to_string()],
        ]
    }

    fn parse_upload() -> AuthorizationClaim {
        AuthorizationClaim::parse("Upload Blob", NOW - 10, &tags("upload", NOW + 60)).unwrap()
    }

    fn validation(
        target: AuthorizationTarget,
        server_requirement: ServerScopeRequirement,
    ) -> AuthorizationValidation {
        AuthorizationValidation::new(
            target,
            server(),
            server_requirement,
            NOW,
            RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS,
        )
        .unwrap()
    }

    #[test]
    fn actions_are_strict_lowercase_wire_verbs() {
        let cases = [
            ("get", AuthorizationAction::Get),
            ("upload", AuthorizationAction::Upload),
            ("list", AuthorizationAction::List),
            ("delete", AuthorizationAction::Delete),
            ("media", AuthorizationAction::Media),
        ];
        for (wire, action) in cases {
            assert_eq!(AuthorizationAction::parse(wire), Ok(action));
            assert_eq!(wire.parse(), Ok(action));
            assert_eq!(action.as_str(), wire);
            assert_eq!(action.to_string(), wire);
        }
        assert_eq!(
            AuthorizationAction::parse("Upload"),
            Err(Error::InvalidAuthorizationAction)
        );
    }

    #[test]
    fn domains_accept_lowercase_ldh_dns_localhost_and_canonical_ipv4() {
        for value in [
            "localhost",
            "media.example.com",
            "xn--bcher-kva.example",
            "127.0.0.1",
            "0.0.0.0",
        ] {
            let domain = ServerDomain::parse(value).unwrap();
            assert_eq!(domain.as_str(), value);
            assert_eq!(domain.to_string(), value);
            assert_eq!(value.parse::<ServerDomain>().unwrap(), domain);
        }

        let long_label = format!("{}.example", "a".repeat(64));
        let long_domain = "a".repeat(254);
        for value in [
            "",
            "Media.example.com",
            "média.example",
            "media_example.com",
            "-media.example",
            "media-.example",
            "media..example",
            "media.example.",
            "https://media.example",
            "media.example:443",
            "127.1",
            "127.00.0.1",
            "256.0.0.1",
            "2130706433",
            &long_label,
            &long_domain,
        ] {
            assert_eq!(
                ServerDomain::parse(value),
                Err(Error::InvalidAuthorizationServerDomain),
                "{value}"
            );
        }
    }

    #[test]
    fn content_requires_bounded_trimmed_human_readable_text() {
        for value in ["", " Upload Blob", "Upload Blob ", "Upload\0Blob", "\u{7f}"] {
            assert_eq!(
                AuthorizationContent::parse(value),
                Err(Error::InvalidAuthorizationContent)
            );
        }
        assert_eq!(
            AuthorizationContent::parse(&"a".repeat(4_097)),
            Err(Error::InvalidAuthorizationContent)
        );
        for value in ["Upload\nBlob", "Upload\tBlob", "Upload\rBlob"] {
            assert_eq!(AuthorizationContent::parse(value).unwrap().as_str(), value);
        }
        let value = AuthorizationContent::parse("Téléverser l'image").unwrap();
        assert_eq!(value.as_str(), "Téléverser l'image");
        assert_eq!(value.to_string(), "Téléverser l'image");
        assert_eq!(
            "Upload Blob".parse::<AuthorizationContent>().unwrap(),
            content()
        );
    }

    #[test]
    fn targets_derive_protocol_actions_hashes_and_requirements() {
        let hash = hash();
        let cases = [
            (
                AuthorizationTarget::GetBlob(hash),
                AuthorizationAction::Get,
                Some(hash),
                false,
            ),
            (
                AuthorizationTarget::Upload(hash),
                AuthorizationAction::Upload,
                Some(hash),
                true,
            ),
            (
                AuthorizationTarget::List,
                AuthorizationAction::List,
                None,
                false,
            ),
            (
                AuthorizationTarget::DeleteBlob(hash),
                AuthorizationAction::Delete,
                Some(hash),
                true,
            ),
            (
                AuthorizationTarget::Mirror(hash),
                AuthorizationAction::Upload,
                Some(hash),
                true,
            ),
            (
                AuthorizationTarget::Media(hash),
                AuthorizationAction::Media,
                Some(hash),
                true,
            ),
        ];
        for (target, action, implied_hash, requires_hash_tag) in cases {
            assert_eq!(target.action(), action);
            assert_eq!(target.implied_hash(), implied_hash);
            assert_eq!(target.requires_hash_tag(), requires_hash_tag);
        }
    }

    #[test]
    fn validation_policy_is_bounded_and_exposes_injected_values() {
        let target = AuthorizationTarget::Upload(hash());
        let policy = validation(target, ServerScopeRequirement::RequiredAnyMatch);
        assert_eq!(policy.target(), target);
        assert_eq!(policy.target_server(), &server());
        assert_eq!(
            policy.server_scope_requirement(),
            ServerScopeRequirement::RequiredAnyMatch
        );
        assert_eq!(policy.now(), NOW);
        assert_eq!(policy.max_created_age_seconds(), Some(300));
        assert_eq!(policy.max_lifetime_seconds(), Some(300));
        assert!(
            AuthorizationValidation::new(
                target,
                server(),
                ServerScopeRequirement::OptionalAnyMatch,
                NOW,
                0,
            )
            .is_ok()
        );
        assert_eq!(
            AuthorizationValidation::new(
                target,
                server(),
                ServerScopeRequirement::OptionalAnyMatch,
                NOW,
                301,
            ),
            Err(Error::InvalidAuthorizationCreatedAge)
        );
    }

    #[test]
    fn tolerant_parser_accepts_multiple_scopes_trailing_fields_and_unknown_tags() {
        let mut tags = tags("upload", NOW + 60);
        tags[0].push("ignored".to_string());
        tags[1].push("ignored".to_string());
        tags[2].push("ignored".to_string());
        tags[3].push("ignored".to_string());
        tags.push(vec!["server".to_string(), "other.example".to_string()]);
        tags.push(vec!["x".to_string(), Sha256::digest(b"other").to_hex()]);
        tags.push(vec!["unknown".to_string()]);
        tags.push(Vec::new());

        let parsed = AuthorizationClaim::parse("Upload Blob", NOW - 10, &tags).unwrap();
        assert_eq!(parsed.content(), &content());
        assert_eq!(parsed.created_at(), NOW - 10);
        assert_eq!(parsed.action(), AuthorizationAction::Upload);
        assert_eq!(parsed.expiration(), NOW + 60);
        assert_eq!(parsed.server_domains().len(), 2);
        assert_eq!(parsed.hashes().len(), 2);
    }

    #[test]
    fn tolerant_parser_rejects_missing_duplicate_and_malformed_known_tags() {
        let cases = [
            (
                vec![vec!["expiration".to_string(), (NOW + 60).to_string()]],
                Error::MissingAuthorizationActionTag,
            ),
            (
                vec![vec!["t".to_string(), "upload".to_string()]],
                Error::MissingAuthorizationExpirationTag,
            ),
            (
                vec![
                    vec!["t".to_string(), "upload".to_string()],
                    vec!["t".to_string(), "upload".to_string()],
                    vec!["expiration".to_string(), (NOW + 60).to_string()],
                ],
                Error::DuplicateAuthorizationActionTag,
            ),
            (
                vec![
                    vec!["t".to_string(), "upload".to_string()],
                    vec!["expiration".to_string(), (NOW + 60).to_string()],
                    vec!["expiration".to_string(), (NOW + 60).to_string()],
                ],
                Error::DuplicateAuthorizationExpirationTag,
            ),
            (
                vec![
                    vec!["t".to_string()],
                    vec!["expiration".to_string(), (NOW + 60).to_string()],
                ],
                Error::MalformedAuthorizationActionTag,
            ),
            (
                vec![
                    vec!["t".to_string(), "upload".to_string()],
                    vec!["expiration".to_string()],
                ],
                Error::MalformedAuthorizationExpirationTag,
            ),
            (
                vec![
                    vec!["t".to_string(), "upload".to_string()],
                    vec!["expiration".to_string(), (NOW + 60).to_string()],
                    vec!["server".to_string()],
                ],
                Error::MalformedAuthorizationServerTag,
            ),
            (
                vec![
                    vec!["t".to_string(), "upload".to_string()],
                    vec!["expiration".to_string(), (NOW + 60).to_string()],
                    vec!["x".to_string()],
                ],
                Error::MalformedAuthorizationHashTag,
            ),
        ];
        for (tags, expected) in cases {
            assert_eq!(
                AuthorizationClaim::parse("Upload Blob", NOW, &tags),
                Err(expected)
            );
        }

        for expiration in ["", " 10", "+10", "ten", "18446744073709551616"] {
            let tags = vec![
                vec!["t".to_string(), "upload".to_string()],
                vec!["expiration".to_string(), expiration.to_string()],
            ];
            assert_eq!(
                AuthorizationClaim::parse("Upload Blob", NOW, &tags),
                Err(Error::MalformedAuthorizationExpirationTag)
            );
        }

        let mut invalid = tags("Upload", NOW + 60);
        assert_eq!(
            AuthorizationClaim::parse("Upload Blob", NOW, &invalid),
            Err(Error::MalformedAuthorizationActionTag)
        );
        invalid = tags("upload", NOW + 60);
        invalid[3][1] = "Media.example".to_string();
        assert_eq!(
            AuthorizationClaim::parse("Upload Blob", NOW, &invalid),
            Err(Error::MalformedAuthorizationServerTag)
        );
        invalid = tags("upload", NOW + 60);
        invalid[2][1] = "not-a-hash".to_string();
        assert_eq!(
            AuthorizationClaim::parse("Upload Blob", NOW, &invalid),
            Err(Error::MalformedAuthorizationHashTag)
        );
    }

    #[test]
    fn upload_validation_enforces_time_action_and_required_scopes() {
        let target = AuthorizationTarget::Upload(hash());
        let required = validation(target, ServerScopeRequirement::RequiredAnyMatch);
        let validated = parse_upload().validate(&required).unwrap();
        assert_eq!(validated.claim(), &parse_upload());
        assert_eq!(validated.target(), target);
        assert_eq!(validated.target_server(), &server());
        assert_eq!(validated.content(), &content());
        assert_eq!(validated.created_at(), NOW - 10);
        assert_eq!(validated.action(), AuthorizationAction::Upload);
        assert_eq!(validated.expiration(), NOW + 60);
        assert_eq!(validated.server_domains(), &[server()]);
        assert_eq!(validated.hashes(), &[hash()]);

        let time_cases = [
            (NOW, NOW + 60, Error::AuthorizationCreatedInFuture),
            (NOW + 1, NOW + 60, Error::AuthorizationCreatedInFuture),
            (NOW - 301, NOW - 1, Error::AuthorizationStale),
            (NOW - 10, NOW - 10, Error::InvalidAuthorizationLifetime),
            (NOW - 10, NOW + 291, Error::InvalidAuthorizationLifetime),
            (NOW - 10, NOW, Error::AuthorizationExpired),
        ];
        for (created_at, expiration, expected) in time_cases {
            let parsed =
                AuthorizationClaim::parse("Upload Blob", created_at, &tags("upload", expiration))
                    .unwrap();
            assert_eq!(parsed.validate(&required), Err(expected));
        }

        let wrong_action =
            AuthorizationClaim::parse("List Blobs", NOW - 10, &tags("list", NOW + 60)).unwrap();
        assert_eq!(
            wrong_action.validate(&required),
            Err(Error::AuthorizationActionMismatch)
        );
    }

    #[test]
    fn bud11_validation_keeps_radroots_replay_limits_out_of_protocol_semantics() {
        let target = AuthorizationTarget::List;
        let policy = AuthorizationValidation::bud11(target, server(), NOW);
        assert_eq!(policy.max_created_age_seconds(), None);
        assert_eq!(policy.max_lifetime_seconds(), None);
        assert_eq!(
            policy.server_scope_requirement(),
            ServerScopeRequirement::OptionalAnyMatch
        );

        let claim = AuthorizationClaim::parse(
            "List archived blobs",
            NOW - 1_000,
            &[
                vec!["t".to_string(), "list".to_string()],
                vec!["expiration".to_string(), (NOW + 1_000).to_string()],
            ],
        )
        .unwrap();
        assert!(claim.validate(&policy).is_ok());
    }

    #[test]
    fn server_scope_uses_optional_or_required_any_match() {
        let target = AuthorizationTarget::Upload(hash());
        let optional = validation(target, ServerScopeRequirement::OptionalAnyMatch);
        let required = validation(target, ServerScopeRequirement::RequiredAnyMatch);

        let no_server =
            AuthorizationClaim::parse("Upload Blob", NOW - 10, &tags("upload", NOW + 60)[..3])
                .unwrap();
        assert!(no_server.validate(&optional).is_ok());
        assert_eq!(
            no_server.validate(&required),
            Err(Error::AuthorizationServerRequired)
        );

        let mut mismatched_tags = tags("upload", NOW + 60);
        mismatched_tags[3][1] = "other.example".to_string();
        let mismatch =
            AuthorizationClaim::parse("Upload Blob", NOW - 10, &mismatched_tags).unwrap();
        assert_eq!(
            mismatch.validate(&optional),
            Err(Error::AuthorizationServerMismatch)
        );

        mismatched_tags.push(vec!["server".to_string(), "media.example.com".to_string()]);
        let any_match =
            AuthorizationClaim::parse("Upload Blob", NOW - 10, &mismatched_tags).unwrap();
        assert!(any_match.validate(&required).is_ok());
    }

    #[test]
    fn hash_scope_follows_each_endpoint_requirement_with_any_match() {
        let hash = hash();
        let no_scope = vec![
            vec!["t".to_string(), "get".to_string()],
            vec!["expiration".to_string(), (NOW + 60).to_string()],
        ];
        let get = AuthorizationClaim::parse("Get Blob", NOW - 10, &no_scope).unwrap();
        assert!(
            get.validate(&validation(
                AuthorizationTarget::GetBlob(hash),
                ServerScopeRequirement::OptionalAnyMatch,
            ))
            .is_ok()
        );

        let list = AuthorizationClaim::parse(
            "List Blobs",
            NOW - 10,
            &[
                vec!["t".to_string(), "list".to_string()],
                vec!["expiration".to_string(), (NOW + 60).to_string()],
                vec!["x".to_string(), HASH_HEX.to_string()],
            ],
        )
        .unwrap();
        assert!(
            list.validate(&validation(
                AuthorizationTarget::List,
                ServerScopeRequirement::OptionalAnyMatch,
            ))
            .is_ok()
        );

        let missing_required = AuthorizationClaim::parse(
            "Upload Blob",
            NOW - 10,
            &[
                vec!["t".to_string(), "upload".to_string()],
                vec!["expiration".to_string(), (NOW + 60).to_string()],
            ],
        )
        .unwrap();
        assert_eq!(
            missing_required.validate(&validation(
                AuthorizationTarget::Upload(hash),
                ServerScopeRequirement::OptionalAnyMatch,
            )),
            Err(Error::AuthorizationHashRequired)
        );

        let wrong_hash = Sha256::digest(b"wrong");
        let wrong_scope = AuthorizationClaim::parse(
            "Get Blob",
            NOW - 10,
            &[
                vec!["t".to_string(), "get".to_string()],
                vec!["expiration".to_string(), (NOW + 60).to_string()],
                vec!["x".to_string(), wrong_hash.to_hex()],
            ],
        )
        .unwrap();
        assert_eq!(
            wrong_scope.validate(&validation(
                AuthorizationTarget::GetBlob(hash),
                ServerScopeRequirement::OptionalAnyMatch,
            )),
            Err(Error::AuthorizationHashMismatch)
        );

        let mut any_tags = tags("delete", NOW + 60);
        any_tags[2][1] = wrong_hash.to_hex();
        any_tags.push(vec!["x".to_string(), HASH_HEX.to_string()]);
        let any_match = AuthorizationClaim::parse("Delete Blob", NOW - 10, &any_tags).unwrap();
        for target in [
            AuthorizationTarget::DeleteBlob(hash),
            AuthorizationTarget::Mirror(hash),
            AuthorizationTarget::Media(hash),
        ] {
            let action = target.action();
            let mut action_tags = any_tags.clone();
            action_tags[0][1] = action.as_str().to_string();
            let parsed =
                AuthorizationClaim::parse("Authorized Operation", NOW - 10, &action_tags).unwrap();
            assert!(
                parsed
                    .validate(&validation(
                        target,
                        ServerScopeRequirement::RequiredAnyMatch,
                    ))
                    .is_ok()
            );
        }
        assert_eq!(any_match.hashes().len(), 2);
    }

    #[test]
    fn strict_authored_upload_claim_emits_canonical_wire_parts() {
        let claim = AuthoredUploadClaim::new(content(), server(), hash(), NOW, 60).unwrap();
        assert_eq!(claim.content(), &content());
        assert_eq!(claim.server_domain(), &server());
        assert_eq!(claim.sha256(), hash());
        assert_eq!(claim.created_at(), NOW);
        assert_eq!(claim.expiration(), NOW + 60);
        assert_eq!(claim.lifetime_seconds(), 60);

        let wire = claim.wire_parts();
        assert_eq!(wire.kind(), RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND);
        assert_eq!(wire.created_at(), NOW);
        assert_eq!(wire.content(), "Upload Blob");
        assert_eq!(wire.tags(), tags("upload", NOW + 60));

        let (kind, created_at, tags, content) = claim.into_wire_parts().into_parts();
        assert_eq!(kind, RADROOTS_BLOSSOM_AUTHORIZATION_EVENT_KIND);
        assert_eq!(created_at, NOW);
        assert_eq!(tags, self::tags("upload", NOW + 60));
        assert_eq!(content, "Upload Blob");
    }

    #[test]
    fn strict_authored_upload_claim_rejects_bad_lifetime_and_overflow() {
        for lifetime in [0, 301] {
            assert_eq!(
                AuthoredUploadClaim::new(content(), server(), hash(), NOW, lifetime,),
                Err(Error::InvalidAuthorizationLifetime)
            );
        }
        assert_eq!(
            AuthoredUploadClaim::new(content(), server(), hash(), u64::MAX, 1,),
            Err(Error::AuthorizationTimestampOverflow)
        );
    }

    #[test]
    fn validation_enforces_exact_time_boundaries() {
        let boundary_tags = tags("upload", NOW + 1);
        let parsed = AuthorizationClaim::parse(
            "Upload Blob",
            NOW - (RADROOTS_BLOSSOM_AUTH_MAX_CREATED_AGE_SECONDS - 1),
            &boundary_tags,
        )
        .unwrap();
        assert!(
            parsed
                .validate(&validation(
                    AuthorizationTarget::Upload(hash()),
                    ServerScopeRequirement::RequiredAnyMatch,
                ))
                .is_ok()
        );

        let zero_age = AuthorizationValidation::new(
            AuthorizationTarget::Upload(hash()),
            server(),
            ServerScopeRequirement::RequiredAnyMatch,
            NOW,
            0,
        )
        .unwrap();
        let created_now = AuthorizationClaim::parse(
            "Upload Blob",
            NOW,
            &tags("upload", NOW + RADROOTS_BLOSSOM_AUTH_MAX_HORIZON_SECONDS),
        )
        .unwrap();
        assert_eq!(
            created_now.validate(&zero_age),
            Err(Error::AuthorizationCreatedInFuture)
        );
    }
}
