//! Concrete resolver implementations for opt-in external validation:
//! filesystem, HTTP, and disk-caching HTTP resolvers. The resolution traits
//! and error types they implement live in the parent [`super`] module.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use std::io::Read;

use sbol_core::document::RawDocument;

use super::{
    ContentResolver, DocumentResolver, ResolutionError, ResolutionErrorKind, ResolvedContent,
};
use crate::{Document, Iri, Resource};

/// File-backed resolver for opt-in local validation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileResolver {
    roots: Vec<PathBuf>,
}

impl FileResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.roots.push(root.into());
        self
    }

    pub fn add_root(&mut self, root: impl Into<PathBuf>) {
        self.roots.push(root.into());
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    fn candidate_paths(&self, iri: &Iri) -> Result<Vec<PathBuf>, ResolutionError> {
        let value = iri.as_str();
        if let Some(path) = file_iri_path(value)? {
            return Ok(vec![path]);
        }

        let Some(relative_path) = iri_path_suffix(value) else {
            return Err(ResolutionError::new(
                ResolutionErrorKind::UnsupportedScheme,
                format!("unsupported source URI scheme for `{value}`"),
            ));
        };

        if self.roots.is_empty() {
            return Err(ResolutionError::new(
                ResolutionErrorKind::UnsupportedScheme,
                format!("no filesystem root configured for `{value}`"),
            ));
        }

        Ok(self
            .roots
            .iter()
            .map(|root| root.join(&relative_path))
            .collect())
    }

    fn read_bytes(&self, iri: &Iri) -> Result<Vec<u8>, ResolutionError> {
        let candidates = self.candidate_paths(iri)?;
        let mut saw_candidate = false;
        for path in candidates {
            saw_candidate = true;
            match fs::read(&path) {
                Ok(bytes) => return Ok(bytes),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(ResolutionError::new(
                        ResolutionErrorKind::Io,
                        format!("could not read `{}`: {error}", path.display()),
                    ));
                }
            }
        }

        let kind = if saw_candidate {
            ResolutionErrorKind::NotFound
        } else {
            ResolutionErrorKind::UnsupportedScheme
        };
        Err(ResolutionError::new(
            kind,
            format!("no filesystem content found for `{}`", iri.as_str()),
        ))
    }
}

impl ContentResolver for FileResolver {
    fn resolve_content(&self, source: &Iri) -> Result<ResolvedContent, ResolutionError> {
        let bytes = self.read_bytes(source)?;
        Ok(ResolvedContent::new(bytes, media_type_for(source.as_str())))
    }
}

impl DocumentResolver for FileResolver {
    fn resolve_document(&self, resource: &Resource) -> Result<RawDocument, ResolutionError> {
        let Some(iri) = resource.as_iri() else {
            return Err(ResolutionError::new(
                ResolutionErrorKind::UnsupportedScheme,
                format!("blank node `{resource}` cannot be resolved as a document"),
            ));
        };
        let bytes = self.read_bytes(iri)?;
        let text = String::from_utf8(bytes).map_err(|error| {
            ResolutionError::new(
                ResolutionErrorKind::InvalidData,
                format!("resolved document `{iri}` was not UTF-8: {error}"),
            )
        })?;
        Document::read_turtle(&text)
            .map(Document::into_raw)
            .map_err(|error| {
                ResolutionError::new(
                    ResolutionErrorKind::Parse,
                    format!("resolved document `{iri}` was not valid Turtle: {error}"),
                )
            })
    }
}

/// HTTP(S) resolver for opt-in external validation.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct HttpResolver {
    agent: ureq::Agent,
}

impl Default for HttpResolver {
    fn default() -> Self {
        Self {
            agent: ureq::Agent::new_with_defaults(),
        }
    }
}

impl HttpResolver {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ContentResolver for HttpResolver {
    fn resolve_content(&self, source: &Iri) -> Result<ResolvedContent, ResolutionError> {
        let value = source.as_str();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            return Err(ResolutionError::new(
                ResolutionErrorKind::UnsupportedScheme,
                format!("unsupported HTTP source URI scheme for `{value}`"),
            ));
        }

        let mut response = self
            .agent
            .get(value)
            .call()
            .map_err(|error| http_error(error, value))?;
        let media_type = response
            .headers()
            .get("content-type")
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.split(';').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .read_to_end(&mut bytes)
            .map_err(ResolutionError::from)?;
        Ok(ResolvedContent::new(bytes, media_type))
    }
}

impl DocumentResolver for HttpResolver {
    fn resolve_document(&self, resource: &Resource) -> Result<RawDocument, ResolutionError> {
        let Some(iri) = resource.as_iri() else {
            return Err(ResolutionError::new(
                ResolutionErrorKind::UnsupportedScheme,
                format!("blank node `{resource}` cannot be resolved as a document"),
            ));
        };
        let content = self.resolve_content(iri)?;
        let text = String::from_utf8(content.bytes).map_err(|error| {
            ResolutionError::new(
                ResolutionErrorKind::InvalidData,
                format!("resolved document `{iri}` was not UTF-8: {error}"),
            )
        })?;
        Document::read_turtle(&text)
            .map(Document::into_raw)
            .map_err(|error| {
                ResolutionError::new(
                    ResolutionErrorKind::Parse,
                    format!("resolved document `{iri}` was not valid Turtle: {error}"),
                )
            })
    }
}

/// Decorator that caches `HttpResolver` results on disk.
///
/// The cache directory mirrors:
/// `<cache_dir>/<sha256(source_iri)>/{content,media_type}`.
///
/// Cache hits are deterministic across CI runs (matters for sbol3-12805
/// hash verification, where re-fetching the source on every run would
/// produce flaky results when upstream content changes). Writes are
/// atomic via tmp-file + rename so a crashed run never leaves a corrupt
/// cache entry.
#[derive(Debug)]
pub struct CachingHttpResolver {
    inner: HttpResolver,
    cache_dir: PathBuf,
}

impl CachingHttpResolver {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            inner: HttpResolver::new(),
            cache_dir: cache_dir.into(),
        }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    fn cache_paths(&self, source: &Iri) -> (PathBuf, PathBuf) {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        hasher.update(source.as_str().as_bytes());
        let digest = hex_digest_static(&hasher.finalize());
        let dir = self.cache_dir.join(&digest);
        (dir.join("content"), dir.join("media_type"))
    }

    fn read_cached(&self, source: &Iri) -> Option<ResolvedContent> {
        let (content_path, media_type_path) = self.cache_paths(source);
        let bytes = fs::read(&content_path).ok()?;
        let media_type = fs::read_to_string(&media_type_path).ok().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        });
        Some(ResolvedContent::new(bytes, media_type))
    }

    fn write_cached(&self, source: &Iri, content: &ResolvedContent) -> io::Result<()> {
        let (content_path, media_type_path) = self.cache_paths(source);
        if let Some(parent) = content_path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic(&content_path, &content.bytes)?;
        let media_type = content.media_type.as_deref().unwrap_or("");
        write_atomic(&media_type_path, media_type.as_bytes())
    }
}

impl ContentResolver for CachingHttpResolver {
    fn resolve_content(&self, source: &Iri) -> Result<ResolvedContent, ResolutionError> {
        if let Some(cached) = self.read_cached(source) {
            return Ok(cached);
        }
        let content = self.inner.resolve_content(source)?;
        let _ = self.write_cached(source, &content);
        Ok(content)
    }
}

impl DocumentResolver for CachingHttpResolver {
    fn resolve_document(&self, resource: &Resource) -> Result<RawDocument, ResolutionError> {
        let Some(iri) = resource.as_iri() else {
            return Err(ResolutionError::new(
                ResolutionErrorKind::UnsupportedScheme,
                format!("blank node `{resource}` cannot be resolved as a document"),
            ));
        };
        let content = self.resolve_content(iri)?;
        let text = String::from_utf8(content.bytes).map_err(|error| {
            ResolutionError::new(
                ResolutionErrorKind::InvalidData,
                format!("resolved document `{iri}` was not UTF-8: {error}"),
            )
        })?;
        Document::read_turtle(&text)
            .map(Document::into_raw)
            .map_err(|error| {
                ResolutionError::new(
                    ResolutionErrorKind::Parse,
                    format!("resolved document `{iri}` was not valid Turtle: {error}"),
                )
            })
    }
}

fn hex_digest_static(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    use std::fmt::Write as _;
    for byte in bytes {
        write!(out, "{byte:02x}").unwrap();
    }
    out
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

fn http_error(error: ureq::Error, url: &str) -> ResolutionError {
    match error {
        ureq::Error::StatusCode(404) => {
            ResolutionError::new(ResolutionErrorKind::NotFound, "HTTP resource was not found")
        }
        ureq::Error::StatusCode(status) => ResolutionError::new(
            ResolutionErrorKind::Http,
            format!("HTTP {status} while resolving {url}"),
        ),
        error => ResolutionError::new(
            ResolutionErrorKind::Http,
            format!("HTTP transport error: {error}"),
        ),
    }
}

fn file_iri_path(value: &str) -> Result<Option<PathBuf>, ResolutionError> {
    let Some(rest) = value.strip_prefix("file://") else {
        return Ok(None);
    };
    if rest.is_empty() {
        return Err(ResolutionError::new(
            ResolutionErrorKind::InvalidData,
            "empty file URI",
        ));
    }
    if rest.starts_with('/') {
        return Ok(Some(PathBuf::from(percent_decode(rest)?)));
    }
    if let Some(path) = rest.strip_prefix("localhost/") {
        return Ok(Some(PathBuf::from(format!("/{}", percent_decode(path)?))));
    }
    Err(ResolutionError::new(
        ResolutionErrorKind::UnsupportedScheme,
        format!("unsupported non-local file URI `{value}`"),
    ))
}

fn iri_path_suffix(value: &str) -> Option<PathBuf> {
    let path = if let Some((_, rest)) = value.split_once("://") {
        rest.split_once('/').map(|(_, path)| path)?
    } else {
        value
    };
    let path = path.trim_start_matches('/');
    if path.is_empty() || path.contains("..") {
        return None;
    }
    Some(Path::new(path).to_path_buf())
}

fn percent_decode(value: &str) -> Result<String, ResolutionError> {
    let mut decoded = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(hex) = value.get(index + 1..index + 3) else {
                return Err(ResolutionError::new(
                    ResolutionErrorKind::InvalidData,
                    format!("invalid percent escape in `{value}`"),
                ));
            };
            let byte = u8::from_str_radix(hex, 16).map_err(|error| {
                ResolutionError::new(
                    ResolutionErrorKind::InvalidData,
                    format!("invalid percent escape in `{value}`: {error}"),
                )
            })?;
            decoded.push(byte as char);
            index += 3;
        } else {
            decoded.push(bytes[index] as char);
            index += 1;
        }
    }
    Ok(decoded)
}

fn media_type_for(value: &str) -> Option<String> {
    let extension = value.rsplit('.').next()?.to_ascii_lowercase();
    let media_type = match extension.as_str() {
        "csv" => "text/csv",
        "json" => "application/json",
        "nt" => "application/n-triples",
        "ttl" | "turtle" => "text/turtle",
        "txt" => "text/plain",
        "xml" => "application/xml",
        _ => return None,
    };
    Some(media_type.to_owned())
}
