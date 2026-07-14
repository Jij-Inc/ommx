//! Stable OMMX-owned errors for remote Artifact lookup.

use super::remote_transport::{
    InvalidAuthenticationConfiguration, InvalidRemoteResponse, RemoteTransportFailure,
};
use super::ImageRef;
use oci_client::errors::{OciDistributionError, OciErrorCode};

/// Failure while looking up or importing an Artifact from a remote registry.
///
/// Remote Artifact APIs keep returning [`crate::Result`]. This signal is
/// stored in the returned [`crate::Error`] chain, so callers that need to
/// recover from a specific remote failure can use
/// [`crate::Error::downcast_ref`] without depending on the OCI transport
/// implementation. The source chain retains the original registry and
/// transport details.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RemoteArtifactError {
    /// The requested repository, manifest, or tag does not exist.
    #[error("Remote artifact manifest not found: {image}")]
    ManifestNotFound {
        /// The exact remote reference that was requested.
        image: Box<ImageRef>,
        /// Registry and transport context for the failed lookup.
        #[source]
        source: crate::Error,
    },
    /// Authentication credentials were missing, rejected, or malformed.
    #[error("Failed to authenticate for remote artifact: {image}")]
    Authentication {
        /// The exact remote reference that was requested.
        image: Box<ImageRef>,
        /// Registry and transport context for the authentication failure.
        #[source]
        source: crate::Error,
    },
    /// The authenticated or anonymous caller is not authorized to read the Artifact.
    #[error("Access denied for remote artifact: {image}")]
    Authorization {
        /// The exact remote reference that was requested.
        image: Box<ImageRef>,
        /// Registry context for the authorization failure.
        #[source]
        source: crate::Error,
    },
    /// The registry could not be reached or returned a server-side failure.
    #[error("Transport failure while accessing remote artifact: {image}")]
    Transport {
        /// The exact remote reference that was requested.
        image: Box<ImageRef>,
        /// Network or registry-server context for the failure.
        #[source]
        source: crate::Error,
    },
    /// The remote response does not describe a valid OMMX Artifact.
    #[error("Invalid remote artifact: {image}")]
    InvalidArtifact {
        /// The exact remote reference that was requested.
        image: Box<ImageRef>,
        /// Parsing, digest, or validation context for the invalid Artifact.
        #[source]
        source: crate::Error,
    },
    /// A remote lookup failure that does not fit a more specific stable category.
    #[error("Failed to access remote artifact: {image}")]
    Other {
        /// The exact remote reference that was requested.
        image: Box<ImageRef>,
        /// Original failure context.
        #[source]
        source: crate::Error,
    },
}

impl RemoteArtifactError {
    pub(crate) fn classify(image: &ImageRef, source: crate::Error) -> Self {
        let category = if source.chain().any(|cause| {
            cause
                .downcast_ref::<InvalidAuthenticationConfiguration>()
                .is_some()
        }) {
            Category::Authentication
        } else if source
            .chain()
            .any(|cause| cause.downcast_ref::<InvalidRemoteResponse>().is_some())
        {
            Category::InvalidArtifact
        } else if source
            .chain()
            .any(|cause| cause.downcast_ref::<RemoteTransportFailure>().is_some())
        {
            Category::Transport
        } else if let Some(error) = source
            .chain()
            .find_map(|cause| cause.downcast_ref::<OciDistributionError>())
        {
            classify_oci_error(error)
        } else if source
            .chain()
            .any(|cause| cause.downcast_ref::<serde_json::Error>().is_some())
        {
            Category::InvalidArtifact
        } else {
            Category::Other
        };

        Self::from_category(image, source, category)
    }

    pub(crate) fn invalid_artifact(image: &ImageRef, source: crate::Error) -> Self {
        Self::from_category(image, source, Category::InvalidArtifact)
    }

    fn from_category(image: &ImageRef, source: crate::Error, category: Category) -> Self {
        let image = Box::new(image.clone());
        match category {
            Category::ManifestNotFound => Self::ManifestNotFound { image, source },
            Category::Authentication => Self::Authentication { image, source },
            Category::Authorization => Self::Authorization { image, source },
            Category::Transport => Self::Transport { image, source },
            Category::InvalidArtifact => Self::InvalidArtifact { image, source },
            Category::Other => Self::Other { image, source },
        }
    }

    /// The exact remote reference associated with this failure.
    pub fn image(&self) -> &ImageRef {
        match self {
            Self::ManifestNotFound { image, .. }
            | Self::Authentication { image, .. }
            | Self::Authorization { image, .. }
            | Self::Transport { image, .. }
            | Self::InvalidArtifact { image, .. }
            | Self::Other { image, .. } => image.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    ManifestNotFound,
    Authentication,
    Authorization,
    Transport,
    InvalidArtifact,
    Other,
}

fn classify_oci_error(error: &OciDistributionError) -> Category {
    match error {
        OciDistributionError::ImageManifestNotFoundError(_) => Category::ManifestNotFound,
        OciDistributionError::AuthenticationFailure(_)
        | OciDistributionError::ConfigConversionError(_)
        | OciDistributionError::RegistryTokenDecodeError(_)
        | OciDistributionError::UnauthorizedError { .. } => Category::Authentication,
        OciDistributionError::RegistryError { envelope, .. } => {
            classify_registry_codes(envelope.errors.iter().map(|error| &error.code))
        }
        OciDistributionError::RequestError(_) | OciDistributionError::IoError(_) => {
            Category::Transport
        }
        OciDistributionError::ServerError { code: 401, .. } => Category::Authentication,
        OciDistributionError::ServerError { code: 403, .. } => Category::Authorization,
        OciDistributionError::ServerError { code: 404, .. } => Category::ManifestNotFound,
        OciDistributionError::ServerError { code, .. } if *code >= 500 => Category::Transport,
        OciDistributionError::DigestError(_)
        | OciDistributionError::HeaderValueError(_)
        | OciDistributionError::JsonError(_)
        | OciDistributionError::ManifestEncodingError(_)
        | OciDistributionError::ManifestParsingError(_)
        | OciDistributionError::RegistryNoDigestError
        | OciDistributionError::SpecViolationError(_)
        | OciDistributionError::UnsupportedMediaTypeError(_)
        | OciDistributionError::UnsupportedSchemaVersionError(_)
        | OciDistributionError::VersionedParsingError(_)
        | OciDistributionError::ImageIndexParsingNoPlatformResolverError
        | OciDistributionError::IncompatibleLayerMediaTypeError(_) => Category::InvalidArtifact,
        OciDistributionError::GenericError(_)
        | OciDistributionError::PushNoDataError
        | OciDistributionError::PushLayerNoDataError
        | OciDistributionError::PullNoLayersError
        | OciDistributionError::RegistryNoLocationError
        | OciDistributionError::UrlParseError(_)
        | OciDistributionError::ServerError { .. } => Category::Other,
    }
}

fn classify_registry_codes<'a>(codes: impl Iterator<Item = &'a OciErrorCode>) -> Category {
    let codes: Vec<_> = codes.collect();
    if codes
        .iter()
        .any(|code| matches!(code, OciErrorCode::Unauthorized))
    {
        return Category::Authentication;
    }
    if codes
        .iter()
        .any(|code| matches!(code, OciErrorCode::Denied))
    {
        return Category::Authorization;
    }
    if !codes.is_empty()
        && codes.iter().all(|code| {
            matches!(
                code,
                OciErrorCode::ManifestUnknown | OciErrorCode::NameUnknown | OciErrorCode::NotFound
            )
        })
    {
        return Category::ManifestNotFound;
    }
    if codes.iter().any(|code| {
        matches!(
            code,
            OciErrorCode::DigestInvalid
                | OciErrorCode::ManifestBlobUnknown
                | OciErrorCode::ManifestInvalid
                | OciErrorCode::ManifestUnverified
                | OciErrorCode::NameInvalid
                | OciErrorCode::SizeInvalid
                | OciErrorCode::TagInvalid
        )
    }) {
        return Category::InvalidArtifact;
    }
    Category::Other
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci_client::errors::{OciEnvelope, OciError};

    fn registry_error(codes: impl IntoIterator<Item = OciErrorCode>) -> OciDistributionError {
        OciDistributionError::RegistryError {
            envelope: OciEnvelope {
                errors: codes
                    .into_iter()
                    .map(|code| OciError {
                        code,
                        message: String::new(),
                        detail: serde_json::Value::Null,
                    })
                    .collect(),
            },
            url: "https://registry.example/v2/repo/manifests/tag".to_string(),
        }
    }

    #[test]
    fn classifies_manifest_absence_without_string_matching() {
        assert_eq!(
            classify_oci_error(&registry_error([OciErrorCode::ManifestUnknown])),
            Category::ManifestNotFound
        );
        assert_eq!(
            classify_oci_error(&registry_error([OciErrorCode::NameUnknown])),
            Category::ManifestNotFound
        );
    }

    #[test]
    fn authentication_and_authorization_take_priority_over_absence() {
        assert_eq!(
            classify_oci_error(&registry_error([
                OciErrorCode::ManifestUnknown,
                OciErrorCode::Unauthorized,
            ])),
            Category::Authentication
        );
        assert_eq!(
            classify_oci_error(&registry_error([OciErrorCode::Denied])),
            Category::Authorization
        );
    }

    #[test]
    fn classifies_registry_server_failure_as_transport() {
        assert_eq!(
            classify_oci_error(&OciDistributionError::ServerError {
                code: 503,
                url: "https://registry.example/v2/".to_string(),
                message: "unavailable".to_string(),
            }),
            Category::Transport
        );
    }
}
