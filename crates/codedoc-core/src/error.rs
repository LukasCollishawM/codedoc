use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    #[error("digest must be 64 hexadecimal characters, found {found}")]
    DigestLength { found: usize },

    #[error("digest contains non-hexadecimal characters: {found}")]
    DigestEncoding { found: String },

    #[error("git revision must be 7 to 64 hexadecimal characters, found {found:?}")]
    GitRevision { found: String },

    #[error("symbol path must be language://segment/segment, found {found:?}")]
    SymbolPathShape { found: String },

    #[error("symbol path language must be lowercase alphanumeric, found {found:?}")]
    SymbolPathLanguage { found: String },

    #[error("symbol path segment must not be empty: {found:?}")]
    SymbolPathSegment { found: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum CanonicalError {
    #[error("malformed canonical encoding: {detail}")]
    Malformed { detail: String },

    #[error("floating point values are not representable in canonical form: {found}")]
    FloatRejected { found: String },

    #[error("integer exceeds the signed 64 bit canonical range: {found}")]
    IntegerRange { found: String },

    #[error("duplicate object key {key:?} is ambiguous under canonical ordering")]
    DuplicateKey { key: String },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PathError {
    #[error("path {found:?} escapes the repository root")]
    EscapesRoot { found: String },

    #[error("path {found:?} is absolute where a repository relative path is required")]
    Absolute { found: String },

    #[error("path {found:?} is not valid UTF-8")]
    Encoding { found: String },
}
