use std::{net::SocketAddr, time::Duration};

use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256, descriptor::ByteCommitment};
use reqwest::{
    StatusCode,
    header::{
        ACCEPT, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE,
        LOCATION,
    },
};

use crate::transport::{
    BlossomCancellation, BlossomConfig, BlossomEndpoint, BlossomError, BlossomErrorKind,
    BlossomImageDimensions, BlossomInboundReceipt, BlossomInboundRequest, BlossomPhase,
    BlossomUploadReceipt, BlossomUploadRequest, BlossomUploadTransaction,
};

const MAX_RESOLVED_ADDRESSES: usize = 32;
const MAX_ERROR_RESPONSE_BYTES: usize = 4_096;
const MAX_SERVER_ERROR_CODE_BYTES: usize = 64;
const X_SHA_256: &str = "x-sha-256";
const BLOSSOM_PROBE_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub(crate) struct BlossomProbeObservation {
    pub(crate) http_status: u16,
}

pub(crate) struct BlossomProbeFailure {
    pub(crate) error: BlossomError,
    pub(crate) dns_policy_validated: bool,
}

/// Performs one BUD-01-shaped GET for an impossible sentinel digest.
///
/// The request carries no authorization and cannot upload, delete, or mutate a
/// server. Any terminal HTTP response proves only DNS-policy, transport, and
/// HTTP reachability for the exact configured origin.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn probe(
    config: BlossomConfig,
    endpoint: BlossomEndpoint,
    cancellation: BlossomCancellation,
) -> Result<BlossomProbeObservation, BlossomProbeFailure> {
    let mut url = BlobUrl::parse(format!("{}/{BLOSSOM_PROBE_HASH}", endpoint.origin()).as_str())
        .map_err(|_| BlossomProbeFailure {
            error: failure(
                BlossomErrorKind::InvalidEndpoint,
                BlossomPhase::Probe,
                false,
                false,
                0,
            ),
            dns_policy_validated: false,
        })?;
    let mut dns_policy_validated = false;
    for redirects in 0..=config.max_redirects() {
        let current =
            config
                .profile()
                .endpoint_for_blob(&url)
                .ok_or_else(|| BlossomProbeFailure {
                    error: failure(
                        BlossomErrorKind::UnsafeRedirect,
                        BlossomPhase::Probe,
                        false,
                        false,
                        1,
                    ),
                    dns_policy_validated,
                })?;
        let addresses = resolve(
            current,
            config.connect_timeout(),
            &cancellation,
            BlossomPhase::Probe,
            1,
            false,
        )
        .await
        .map_err(|error| BlossomProbeFailure {
            error,
            dns_policy_validated,
        })?;
        dns_policy_validated = true;
        let client = hardened_client_with_addresses(
            &config,
            current,
            addresses.as_slice(),
            BlossomPhase::Probe,
            1,
            false,
        )
        .map_err(|error| BlossomProbeFailure {
            error,
            dns_policy_validated,
        })?;
        let pending = client
            .get(url.as_str())
            .header(ACCEPT, "*/*")
            .header(ACCEPT_ENCODING, "identity")
            .send();
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(BlossomProbeFailure {
                    error: failure(
                        BlossomErrorKind::Cancelled,
                        BlossomPhase::Probe,
                        true,
                        false,
                        1,
                    ),
                    dns_policy_validated,
                });
            }
            response = pending => response.map_err(|error| BlossomProbeFailure {
                error: request_error(error, BlossomPhase::Probe, false, 1),
                dns_policy_validated,
            })?,
        };
        if response.status().is_redirection() {
            if redirects == config.max_redirects() {
                return Err(BlossomProbeFailure {
                    error: failure(
                        BlossomErrorKind::RedirectLimit,
                        BlossomPhase::Probe,
                        false,
                        false,
                        1,
                    ),
                    dns_policy_validated,
                });
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| BlossomProbeFailure {
                    error: failure(
                        BlossomErrorKind::UnsafeRedirect,
                        BlossomPhase::Probe,
                        false,
                        false,
                        1,
                    ),
                    dns_policy_validated,
                })?;
            let base = reqwest::Url::parse(url.as_str()).map_err(|_| BlossomProbeFailure {
                error: failure(
                    BlossomErrorKind::UnsafeRedirect,
                    BlossomPhase::Probe,
                    false,
                    false,
                    1,
                ),
                dns_policy_validated,
            })?;
            let next = base
                .join(location)
                .ok()
                .and_then(|value| BlobUrl::parse(value.as_str()).ok())
                .filter(|value| {
                    value.hash_path().hash() == url.hash_path().hash()
                        && config.profile().endpoint_for_blob(value).is_some()
                })
                .ok_or_else(|| BlossomProbeFailure {
                    error: failure(
                        BlossomErrorKind::UnsafeRedirect,
                        BlossomPhase::Probe,
                        false,
                        false,
                        1,
                    ),
                    dns_policy_validated,
                })?;
            url = next;
            continue;
        }
        let status = response.status().as_u16();
        read_bounded(
            response,
            config.max_descriptor_bytes(),
            &cancellation,
            BlossomPhase::Probe,
            false,
            1,
        )
        .await
        .map_err(|error| BlossomProbeFailure {
            error,
            dns_policy_validated,
        })?;
        return Ok(BlossomProbeObservation {
            http_status: status,
        });
    }
    Err(BlossomProbeFailure {
        error: failure(
            BlossomErrorKind::RedirectLimit,
            BlossomPhase::Probe,
            false,
            false,
            1,
        ),
        dns_policy_validated,
    })
}

pub(crate) async fn upload(
    transaction: BlossomUploadTransaction,
    authorization: crate::signing::AuthorizationHeader,
    cancellation: BlossomCancellation,
) -> Result<BlossomUploadReceipt, BlossomError> {
    upload_bound_with_authorization(transaction, authorization.as_str(), cancellation).await
}

async fn upload_bound_with_authorization(
    transaction: BlossomUploadTransaction,
    authorization: &str,
    cancellation: BlossomCancellation,
) -> Result<BlossomUploadReceipt, BlossomError> {
    let config = transaction.config().clone();
    let endpoint = transaction.endpoint().clone();
    let expected_url = transaction.expected_url().clone();
    let request = transaction.into_request();
    if request.byte_size() > config.max_blob_bytes() {
        return Err(failure(
            BlossomErrorKind::ResponseTooLarge,
            BlossomPhase::Verification,
            false,
            false,
            0,
        ));
    }
    let mut upload_attempts = 0_u8;
    let descriptor = loop {
        ensure_not_cancelled(&cancellation, BlossomPhase::Upload, upload_attempts, false)?;
        upload_attempts = upload_attempts.saturating_add(1);
        match upload_once(
            &config,
            &endpoint,
            &request,
            authorization,
            &cancellation,
            upload_attempts,
        )
        .await
        {
            Ok(descriptor) => break descriptor,
            Err(error) if error.retryable() && upload_attempts < config.max_attempts() => {
                retry_delay(
                    &config,
                    upload_attempts,
                    &cancellation,
                    BlossomPhase::Upload,
                    true,
                )
                .await?;
            }
            Err(error) => return Err(error),
        }
    };

    let verified_upload = verify_descriptor(&request, &expected_url, descriptor, upload_attempts)?;
    let mut retrieval_attempts = 0_u8;
    let retrieved = loop {
        ensure_not_cancelled(
            &cancellation,
            BlossomPhase::Retrieval,
            upload_attempts.saturating_add(retrieval_attempts),
            true,
        )?;
        retrieval_attempts = retrieval_attempts.saturating_add(1);
        match retrieve_once(
            &config,
            verified_upload.url().as_blob_url().clone(),
            request.sha256(),
            Some(request.media_type()),
            Some(request.byte_size()),
            Some(request.dimensions()),
            &cancellation,
            upload_attempts.saturating_add(retrieval_attempts),
            true,
        )
        .await
        {
            Ok(retrieved) => break retrieved.bytes,
            Err(error) if error.retryable() && retrieval_attempts < config.max_attempts() => {
                retry_delay(
                    &config,
                    retrieval_attempts,
                    &cancellation,
                    BlossomPhase::Retrieval,
                    true,
                )
                .await?;
            }
            Err(error) => return Err(error),
        }
    };

    if retrieved.as_slice() != request.bytes() {
        return Err(failure(
            BlossomErrorKind::RetrievedBytesMismatch,
            BlossomPhase::Verification,
            false,
            true,
            upload_attempts.saturating_add(retrieval_attempts),
        ));
    }
    verify_image(
        retrieved.as_slice(),
        request.media_type(),
        request.dimensions(),
    )
    .map_err(|error| with_operation(error, true, upload_attempts + retrieval_attempts))?;
    let verified_retrieval = verified_upload
        .into_descriptor()
        .approve_reference()
        .and_then(|approved| approved.verify_bytes(retrieved.as_slice(), request.media_type()))
        .map_err(|_| {
            failure(
                BlossomErrorKind::RetrievedBytesMismatch,
                BlossomPhase::Verification,
                false,
                true,
                upload_attempts.saturating_add(retrieval_attempts),
            )
        })?;

    Ok(BlossomUploadReceipt::new(
        verified_retrieval,
        request.dimensions(),
        upload_attempts.saturating_add(retrieval_attempts),
        request.verified_at_unix_ms(),
    ))
}

pub(crate) async fn retrieve(
    config: BlossomConfig,
    request: BlossomInboundRequest,
    cancellation: BlossomCancellation,
) -> Result<BlossomInboundReceipt, BlossomError> {
    if request
        .expected_byte_size()
        .is_some_and(|size| size > config.max_blob_bytes())
    {
        return Err(failure(
            BlossomErrorKind::ResponseTooLarge,
            BlossomPhase::Verification,
            false,
            false,
            0,
        ));
    }
    let fingerprint = config.fingerprint();
    let mut attempts = 0_u8;
    let retrieved = loop {
        ensure_not_cancelled(&cancellation, BlossomPhase::Retrieval, attempts, false)?;
        attempts = attempts.saturating_add(1);
        match retrieve_once(
            &config,
            request.url().clone(),
            request.url().hash_path().hash(),
            request.expected_media_type(),
            request.expected_byte_size(),
            request.expected_dimensions(),
            &cancellation,
            attempts,
            false,
        )
        .await
        {
            Ok(retrieved) => break retrieved,
            Err(error) if error.retryable() && attempts < config.max_attempts() => {
                retry_delay(
                    &config,
                    attempts,
                    &cancellation,
                    BlossomPhase::Retrieval,
                    false,
                )
                .await?;
            }
            Err(error) => return Err(error),
        }
    };
    let commitment = ByteCommitment::from_bytes(retrieved.bytes.as_slice(), retrieved.media_type);
    Ok(BlossomInboundReceipt::new(
        retrieved.final_url,
        commitment,
        retrieved.dimensions,
        std::sync::Arc::from(retrieved.bytes),
        fingerprint,
        attempts,
        crate::transport::blossom_now_unix_ms(),
    ))
}

#[cfg(test)]
async fn upload_with_authorization(
    config: BlossomConfig,
    request: BlossomUploadRequest,
    authorization: &str,
    cancellation: BlossomCancellation,
) -> Result<BlossomUploadReceipt, BlossomError> {
    let slot = crate::transport::BlossomSlot::new();
    slot.configure(config)?;
    let transaction = slot.prepare_upload(request)?;
    upload_bound_with_authorization(transaction, authorization, cancellation).await
}

// Direct DNS/socket/HTTP behavior is verified by the local real-I/O suite;
// deterministic coverage owns the surrounding retry, validation, and durable
// state policy.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn upload_once(
    config: &BlossomConfig,
    endpoint: &BlossomEndpoint,
    request: &BlossomUploadRequest,
    authorization: &str,
    cancellation: &BlossomCancellation,
    attempt: u8,
) -> Result<BlobDescriptor, BlossomError> {
    let client = hardened_client(
        config,
        endpoint,
        cancellation,
        BlossomPhase::Upload,
        attempt,
        false,
    )
    .await?;
    let pending = client
        .put(endpoint.upload_url())
        .header(AUTHORIZATION, authorization)
        .header(X_SHA_256, request.sha256().to_string())
        .header(CONTENT_TYPE, request.media_type().as_str())
        .header(ACCEPT, "application/json")
        .header(ACCEPT_ENCODING, "identity")
        .body(request.bytes().to_vec())
        .send();
    let response = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(failure(
                BlossomErrorKind::Cancelled,
                BlossomPhase::Upload,
                true,
                true,
                attempt,
            ));
        }
        response = pending => response.map_err(|error| request_error(error, BlossomPhase::Upload, true, attempt))?,
    };

    if response.status().is_redirection() {
        return Err(failure(
            BlossomErrorKind::UnsafeRedirect,
            BlossomPhase::Upload,
            false,
            true,
            attempt,
        ));
    }
    if !matches!(response.status(), StatusCode::OK | StatusCode::CREATED) {
        return Err(http_status_response_error(
            response,
            BlossomPhase::Upload,
            true,
            attempt,
            cancellation,
        )
        .await);
    }
    {
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                failure(
                    BlossomErrorKind::InvalidDescriptor,
                    BlossomPhase::Descriptor,
                    false,
                    true,
                    attempt,
                )
            })?;
        if content_type
            .split(';')
            .next()
            .is_none_or(|value| !value.trim().eq_ignore_ascii_case("application/json"))
        {
            return Err(failure(
                BlossomErrorKind::InvalidDescriptor,
                BlossomPhase::Descriptor,
                false,
                true,
                attempt,
            ));
        }
    }
    let bytes = read_bounded(
        response,
        config.max_descriptor_bytes(),
        cancellation,
        BlossomPhase::Descriptor,
        true,
        attempt,
    )
    .await?;
    serde_json::from_slice(bytes.as_slice()).map_err(|_| {
        failure(
            BlossomErrorKind::InvalidDescriptor,
            BlossomPhase::Descriptor,
            false,
            true,
            attempt,
        )
    })
}

fn verify_descriptor(
    request: &BlossomUploadRequest,
    expected_url: &BlobUrl,
    descriptor: BlobDescriptor,
    attempts: u8,
) -> Result<radroots_blossom::ByteVerifiedDescriptor, BlossomError> {
    // `BlobDescriptor` construction already binds `sha256` to the URL hash,
    // so equality of the typed URL proves equality of that hash as well.
    if descriptor.url() != expected_url
        || descriptor.size() != request.byte_size()
        || descriptor.media_type() != request.media_type()
    {
        return Err(failure(
            BlossomErrorKind::DescriptorMismatch,
            BlossomPhase::Descriptor,
            false,
            true,
            attempts,
        ));
    }
    descriptor
        .approve_reference()
        .and_then(|approved| approved.verify_bytes(request.bytes(), request.media_type()))
        .map_err(|_| {
            failure(
                BlossomErrorKind::DescriptorMismatch,
                BlossomPhase::Descriptor,
                false,
                true,
                attempts,
            )
        })
}

#[cfg_attr(coverage_nightly, coverage(off))]
struct RetrievedBytes {
    final_url: BlobUrl,
    bytes: Vec<u8>,
    media_type: MediaType,
    dimensions: BlossomImageDimensions,
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_arguments)]
async fn retrieve_once(
    config: &BlossomConfig,
    mut url: BlobUrl,
    expected_sha256: Sha256,
    expected_media_type: Option<&MediaType>,
    expected_byte_size: Option<u64>,
    expected_dimensions: Option<BlossomImageDimensions>,
    cancellation: &BlossomCancellation,
    attempt: u8,
    possible_orphan: bool,
) -> Result<RetrievedBytes, BlossomError> {
    for redirects in 0..=config.max_redirects() {
        let endpoint = config.profile().endpoint_for_blob(&url).ok_or_else(|| {
            failure(
                BlossomErrorKind::UnsafeRedirect,
                BlossomPhase::Retrieval,
                false,
                possible_orphan,
                attempt,
            )
        })?;
        let client = hardened_client(
            config,
            endpoint,
            cancellation,
            BlossomPhase::Retrieval,
            attempt,
            possible_orphan,
        )
        .await?;
        let pending = client
            .get(url.as_str())
            .header(
                ACCEPT,
                expected_media_type.map_or("image/*", MediaType::as_str),
            )
            .header(ACCEPT_ENCODING, "identity")
            .send();
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(failure(
                    BlossomErrorKind::Cancelled,
                    BlossomPhase::Retrieval,
                    true,
                    possible_orphan,
                    attempt,
                ));
            }
            response = pending => response.map_err(|error| request_error(error, BlossomPhase::Retrieval, possible_orphan, attempt))?,
        };
        if response.status().is_redirection() {
            if redirects == config.max_redirects() {
                return Err(failure(
                    BlossomErrorKind::RedirectLimit,
                    BlossomPhase::Retrieval,
                    false,
                    possible_orphan,
                    attempt,
                ));
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    failure(
                        BlossomErrorKind::UnsafeRedirect,
                        BlossomPhase::Retrieval,
                        false,
                        possible_orphan,
                        attempt,
                    )
                })?;
            let base = reqwest::Url::parse(url.as_str()).map_err(|_| {
                failure(
                    BlossomErrorKind::UnsafeRedirect,
                    BlossomPhase::Retrieval,
                    false,
                    possible_orphan,
                    attempt,
                )
            })?;
            let next = base.join(location).map_err(|_| {
                failure(
                    BlossomErrorKind::UnsafeRedirect,
                    BlossomPhase::Retrieval,
                    false,
                    possible_orphan,
                    attempt,
                )
            })?;
            let next = BlobUrl::parse(next.as_str()).map_err(|_| {
                failure(
                    BlossomErrorKind::UnsafeRedirect,
                    BlossomPhase::Retrieval,
                    false,
                    possible_orphan,
                    attempt,
                )
            })?;
            if next.hash_path().hash() != expected_sha256
                || next.clone().approve().is_err()
                || config.profile().endpoint_for_blob(&next).is_none()
            {
                return Err(failure(
                    BlossomErrorKind::UnsafeRedirect,
                    BlossomPhase::Retrieval,
                    false,
                    possible_orphan,
                    attempt,
                ));
            }
            url = next;
            continue;
        }
        if response.status() != StatusCode::OK {
            return Err(http_status_response_error(
                response,
                BlossomPhase::Retrieval,
                possible_orphan,
                attempt,
                cancellation,
            )
            .await);
        }
        let actual_media_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| MediaType::parse(value).ok())
            .ok_or_else(|| {
                failure(
                    BlossomErrorKind::MediaTypeMismatch,
                    BlossomPhase::Verification,
                    false,
                    possible_orphan,
                    attempt,
                )
            })?;
        if expected_media_type.is_some_and(|expected| expected != &actual_media_type) {
            return Err(failure(
                BlossomErrorKind::MediaTypeMismatch,
                BlossomPhase::Verification,
                false,
                possible_orphan,
                attempt,
            ));
        }
        let canonical_extension =
            crate::transport::canonical_image_extension(&actual_media_type)
                .map_err(|error| with_operation(error, possible_orphan, attempt))?;
        if url
            .hash_path()
            .extension()
            .is_none_or(|value| value.as_str() != canonical_extension)
        {
            return Err(failure(
                BlossomErrorKind::MediaTypeMismatch,
                BlossomPhase::Verification,
                false,
                possible_orphan,
                attempt,
            ));
        }
        if response
            .headers()
            .get(CONTENT_ENCODING)
            .is_some_and(|value| {
                value
                    .to_str()
                    .map_or(true, |encoding| !encoding.eq_ignore_ascii_case("identity"))
            })
        {
            return Err(failure(
                BlossomErrorKind::ContentEncodingDenied,
                BlossomPhase::Retrieval,
                false,
                possible_orphan,
                attempt,
            ));
        }
        let content_length = match response.headers().get(CONTENT_LENGTH) {
            Some(value) => Some(
                value
                    .to_str()
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
                    .ok_or_else(|| {
                        failure(
                            BlossomErrorKind::ResponseSizeMismatch,
                            BlossomPhase::Retrieval,
                            false,
                            possible_orphan,
                            attempt,
                        )
                    })?,
            ),
            None => None,
        };
        if content_length.is_some_and(|size| size > config.max_blob_bytes()) {
            return Err(failure(
                BlossomErrorKind::ResponseTooLarge,
                BlossomPhase::Retrieval,
                false,
                possible_orphan,
                attempt,
            ));
        }
        if expected_byte_size
            .zip(content_length)
            .is_some_and(|(expected, actual)| expected != actual)
        {
            return Err(failure(
                BlossomErrorKind::ResponseSizeMismatch,
                BlossomPhase::Verification,
                false,
                possible_orphan,
                attempt,
            ));
        }
        let bytes = read_bounded(
            response,
            usize::try_from(config.max_blob_bytes()).unwrap_or(usize::MAX),
            cancellation,
            BlossomPhase::Retrieval,
            possible_orphan,
            attempt,
        )
        .await?;
        let byte_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if expected_byte_size.is_some_and(|expected| expected != byte_size)
            || content_length.is_some_and(|expected| expected != byte_size)
        {
            return Err(failure(
                BlossomErrorKind::ResponseSizeMismatch,
                BlossomPhase::Verification,
                false,
                possible_orphan,
                attempt,
            ));
        }
        if Sha256::digest(bytes.as_slice()) != expected_sha256 {
            return Err(failure(
                BlossomErrorKind::ResponseHashMismatch,
                BlossomPhase::Verification,
                false,
                possible_orphan,
                attempt,
            ));
        }
        let dimensions = inspect_image(bytes.as_slice(), &actual_media_type)
            .map_err(|error| with_operation(error, possible_orphan, attempt))?;
        if expected_dimensions.is_some_and(|expected| expected != dimensions) {
            return Err(failure(
                BlossomErrorKind::DimensionMismatch,
                BlossomPhase::Verification,
                false,
                possible_orphan,
                attempt,
            ));
        }
        return Ok(RetrievedBytes {
            final_url: url,
            bytes,
            media_type: actual_media_type,
            dimensions,
        });
    }
    Err(failure(
        BlossomErrorKind::RedirectLimit,
        BlossomPhase::Retrieval,
        false,
        possible_orphan,
        attempt,
    ))
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn hardened_client(
    config: &BlossomConfig,
    endpoint: &BlossomEndpoint,
    cancellation: &BlossomCancellation,
    phase: BlossomPhase,
    attempts: u8,
    possible_orphan: bool,
) -> Result<reqwest::Client, BlossomError> {
    let addresses = resolve(
        endpoint,
        config.connect_timeout(),
        cancellation,
        phase,
        attempts,
        possible_orphan,
    )
    .await?;
    hardened_client_with_addresses(
        config,
        endpoint,
        addresses.as_slice(),
        phase,
        attempts,
        possible_orphan,
    )
}

fn hardened_client_with_addresses(
    config: &BlossomConfig,
    endpoint: &BlossomEndpoint,
    addresses: &[SocketAddr],
    phase: BlossomPhase,
    attempts: u8,
    possible_orphan: bool,
) -> Result<reqwest::Client, BlossomError> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(config.connect_timeout())
        .timeout(config.request_timeout())
        .pool_max_idle_per_host(0);
    if endpoint.host().parse::<std::net::IpAddr>().is_err() {
        let address = addresses.first().ok_or_else(|| {
            failure(
                BlossomErrorKind::ResolutionFailed,
                phase,
                true,
                possible_orphan,
                attempts,
            )
        })?;
        builder = builder.resolve(endpoint.host(), *address);
    }
    builder.build().map_err(|_| {
        failure(
            BlossomErrorKind::Transport,
            phase,
            true,
            possible_orphan,
            attempts,
        )
    })
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn resolve(
    endpoint: &BlossomEndpoint,
    timeout: Duration,
    cancellation: &BlossomCancellation,
    phase: BlossomPhase,
    attempts: u8,
    possible_orphan: bool,
) -> Result<Vec<SocketAddr>, BlossomError> {
    let lookup = tokio::net::lookup_host((endpoint.host(), endpoint.port()));
    let mut resolved = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(failure(
                BlossomErrorKind::Cancelled,
                phase,
                true,
                possible_orphan,
                attempts,
            ));
        }
        result = tokio::time::timeout(timeout, lookup) => result
            .map_err(|_| failure(
                BlossomErrorKind::Timeout,
                phase,
                true,
                possible_orphan,
                attempts,
            ))?
            .map_err(|_| {
            failure(
                BlossomErrorKind::ResolutionFailed,
                phase,
                true,
                possible_orphan,
                attempts,
            )
        })?,
    };
    let addresses = resolved
        .by_ref()
        .take(MAX_RESOLVED_ADDRESSES + 1)
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.len() > MAX_RESOLVED_ADDRESSES {
        return Err(failure(
            BlossomErrorKind::ResolutionFailed,
            phase,
            true,
            possible_orphan,
            attempts,
        ));
    }
    endpoint
        .validate_resolved_addresses(addresses.iter().map(SocketAddr::ip))
        .map_err(|error| with_operation(error, possible_orphan, attempts))?;
    Ok(addresses)
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn read_bounded(
    mut response: reqwest::Response,
    max_bytes: usize,
    cancellation: &BlossomCancellation,
    phase: BlossomPhase,
    possible_orphan: bool,
    attempts: u8,
) -> Result<Vec<u8>, BlossomError> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(failure(
            BlossomErrorKind::ResponseTooLarge,
            phase,
            false,
            possible_orphan,
            attempts,
        ));
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(max_bytes),
    );
    loop {
        let pending = response.chunk();
        let chunk = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(failure(
                    BlossomErrorKind::Cancelled,
                    phase,
                    true,
                    possible_orphan,
                    attempts,
                ));
            }
            chunk = pending => chunk.map_err(|error| request_error(error, phase, possible_orphan, attempts))?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(failure(
                BlossomErrorKind::ResponseTooLarge,
                phase,
                false,
                possible_orphan,
                attempts,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn retry_delay(
    config: &BlossomConfig,
    attempt: u8,
    cancellation: &BlossomCancellation,
    phase: BlossomPhase,
    possible_orphan: bool,
) -> Result<(), BlossomError> {
    let exponent = u32::from(attempt.saturating_sub(1)).min(16);
    let factor = 1_u32 << exponent;
    let delay = config
        .initial_retry_delay()
        .saturating_mul(factor)
        .min(Duration::from_secs(30));
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(failure(
            BlossomErrorKind::Cancelled,
            phase,
            true,
            possible_orphan,
            attempt,
        )),
        _ = tokio::time::sleep(delay) => Ok(()),
    }
}

fn ensure_not_cancelled(
    cancellation: &BlossomCancellation,
    phase: BlossomPhase,
    attempts: u8,
    possible_orphan: bool,
) -> Result<(), BlossomError> {
    if cancellation.is_cancelled() {
        Err(failure(
            BlossomErrorKind::Cancelled,
            phase,
            true,
            possible_orphan,
            attempts,
        ))
    } else {
        Ok(())
    }
}

fn request_error(
    error: reqwest::Error,
    phase: BlossomPhase,
    possible_orphan: bool,
    attempts: u8,
) -> BlossomError {
    let kind = if error.is_timeout() {
        BlossomErrorKind::Timeout
    } else {
        BlossomErrorKind::Transport
    };
    failure(kind, phase, true, possible_orphan, attempts)
}

fn http_status_error(
    status: StatusCode,
    phase: BlossomPhase,
    possible_orphan: bool,
    attempts: u8,
) -> BlossomError {
    let retryable = matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_EARLY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    );
    failure(
        BlossomErrorKind::HttpStatus,
        phase,
        retryable,
        possible_orphan,
        attempts,
    )
    .with_http_status(status.as_u16())
}

async fn http_status_response_error(
    response: reqwest::Response,
    phase: BlossomPhase,
    possible_orphan: bool,
    attempts: u8,
    cancellation: &BlossomCancellation,
) -> BlossomError {
    let status = response.status();
    let mut error = http_status_error(status, phase, possible_orphan, attempts);
    if let Some(code) = read_server_error_code(response, cancellation).await {
        error = error.with_server_error_code(code);
    }
    error
}

async fn read_server_error_code(
    mut response: reqwest::Response,
    cancellation: &BlossomCancellation,
) -> Option<String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ERROR_RESPONSE_BYTES as u64)
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(MAX_ERROR_RESPONSE_BYTES),
    );
    loop {
        let chunk = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return None,
            chunk = response.chunk() => chunk.ok()?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        if bytes.len().saturating_add(chunk.len()) > MAX_ERROR_RESPONSE_BYTES {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_server_error_code(bytes.as_slice())
}

fn parse_server_error_code(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_ERROR_RESPONSE_BYTES {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let code = value.get("error")?.as_str()?;
    if code.is_empty()
        || code.len() > MAX_SERVER_ERROR_CODE_BYTES
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return None;
    }
    Some(code.to_owned())
}

fn with_operation(error: BlossomError, possible_orphan: bool, attempts: u8) -> BlossomError {
    error.with_operation(possible_orphan, attempts)
}

fn failure(
    kind: BlossomErrorKind,
    phase: BlossomPhase,
    retryable: bool,
    possible_orphan: bool,
    attempts: u8,
) -> BlossomError {
    BlossomError::new(kind, phase, retryable, possible_orphan, attempts)
}

pub(crate) fn verify_image(
    bytes: &[u8],
    media_type: &MediaType,
    expected: BlossomImageDimensions,
) -> Result<(), BlossomError> {
    let dimensions = inspect_image(bytes, media_type)?;
    if dimensions != expected {
        return Err(failure(
            BlossomErrorKind::DimensionMismatch,
            BlossomPhase::Verification,
            false,
            false,
            0,
        ));
    }
    Ok(())
}

fn inspect_image(
    bytes: &[u8],
    media_type: &MediaType,
) -> Result<BlossomImageDimensions, BlossomError> {
    let detected = detect_image(bytes).ok_or_else(|| {
        failure(
            BlossomErrorKind::InvalidImageBytes,
            BlossomPhase::Verification,
            false,
            false,
            0,
        )
    })?;
    let declared = match media_type.as_str() {
        "image/png" => ImageKind::Png,
        "image/jpeg" => ImageKind::Jpeg,
        "image/gif" => ImageKind::Gif,
        "image/webp" => ImageKind::Webp,
        _ => {
            return Err(failure(
                BlossomErrorKind::UnsupportedMediaType,
                BlossomPhase::Verification,
                false,
                false,
                0,
            ));
        }
    };
    if declared != detected.0 {
        return Err(failure(
            BlossomErrorKind::MediaTypeMismatch,
            BlossomPhase::Verification,
            false,
            false,
            0,
        ));
    }
    Ok(detected.1)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

fn detect_image(bytes: &[u8]) -> Option<(ImageKind, BlossomImageDimensions)> {
    detect_png(bytes)
        .map(|dimensions| (ImageKind::Png, dimensions))
        .or_else(|| detect_jpeg(bytes).map(|dimensions| (ImageKind::Jpeg, dimensions)))
        .or_else(|| detect_gif(bytes).map(|dimensions| (ImageKind::Gif, dimensions)))
        .or_else(|| detect_webp(bytes).map(|dimensions| (ImageKind::Webp, dimensions)))
}

fn detect_png(bytes: &[u8]) -> Option<BlossomImageDimensions> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    BlossomImageDimensions::new(
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    )
    .ok()
}

fn detect_gif(bytes: &[u8]) -> Option<BlossomImageDimensions> {
    if bytes.len() < 10 || !matches!(&bytes[..6], b"GIF87a" | b"GIF89a") {
        return None;
    }
    BlossomImageDimensions::new(
        u32::from(u16::from_le_bytes(bytes[6..8].try_into().ok()?)),
        u32::from(u16::from_le_bytes(bytes[8..10].try_into().ok()?)),
    )
    .ok()
}

fn detect_jpeg(bytes: &[u8]) -> Option<BlossomImageDimensions> {
    if bytes.len() < 4 || bytes[..2] != [0xff, 0xd8] {
        return None;
    }
    let mut offset = 2_usize;
    while offset < bytes.len() {
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]));
        if length < 2 || offset.checked_add(length)? > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            if length < 7 {
                return None;
            }
            return BlossomImageDimensions::new(
                u32::from(u16::from_be_bytes([
                    *bytes.get(offset + 5)?,
                    *bytes.get(offset + 6)?,
                ])),
                u32::from(u16::from_be_bytes([
                    *bytes.get(offset + 3)?,
                    *bytes.get(offset + 4)?,
                ])),
            )
            .ok();
        }
        offset += length;
    }
    None
}

fn detect_webp(bytes: &[u8]) -> Option<BlossomImageDimensions> {
    if bytes.len() < 30 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    match &bytes[12..16] {
        b"VP8X" => BlossomImageDimensions::new(
            little_u24(&bytes[24..27])?.checked_add(1)?,
            little_u24(&bytes[27..30])?.checked_add(1)?,
        )
        .ok(),
        b"VP8L" if bytes.get(20) == Some(&0x2f) => {
            let packed = u32::from_le_bytes(bytes[21..25].try_into().ok()?);
            BlossomImageDimensions::new(
                (packed & 0x3fff).checked_add(1)?,
                ((packed >> 14) & 0x3fff).checked_add(1)?,
            )
            .ok()
        }
        b"VP8 " if bytes.get(23..26) == Some(&[0x9d, 0x01, 0x2a]) => BlossomImageDimensions::new(
            u32::from(u16::from_le_bytes(bytes[26..28].try_into().ok()?) & 0x3fff),
            u32::from(u16::from_le_bytes(bytes[28..30].try_into().ok()?) & 0x3fff),
        )
        .ok(),
        _ => None,
    }
}

fn little_u24(bytes: &[u8]) -> Option<u32> {
    Some(
        u32::from(*bytes.first()?)
            | u32::from(*bytes.get(1)?) << 8
            | u32::from(*bytes.get(2)?) << 16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_blossom::Sha256;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn image_headers_bind_mime_and_dimensions() {
        let dimensions = BlossomImageDimensions::new(1200, 900).expect("dimensions");
        let bytes = png(1200, 900);
        assert!(verify_image(&bytes, &MediaType::parse("image/png").unwrap(), dimensions).is_ok());
        assert_eq!(
            verify_image(&bytes, &MediaType::parse("image/jpeg").unwrap(), dimensions)
                .expect_err("wrong MIME")
                .kind(),
            BlossomErrorKind::MediaTypeMismatch
        );
        assert_eq!(
            verify_image(
                &bytes,
                &MediaType::parse("image/png").unwrap(),
                BlossomImageDimensions::new(1, 1).unwrap(),
            )
            .expect_err("wrong dimensions")
            .kind(),
            BlossomErrorKind::DimensionMismatch
        );
        assert_eq!(
            verify_image(
                b"not an image",
                &MediaType::parse("image/png").unwrap(),
                dimensions
            )
            .expect_err("invalid image")
            .kind(),
            BlossomErrorKind::InvalidImageBytes
        );
    }

    #[test]
    fn supported_image_headers_are_bounded_and_nonzero() {
        let gif = b"GIF89a\x02\0\x03\0";
        assert_eq!(
            detect_gif(gif),
            Some(BlossomImageDimensions::new(2, 3).unwrap())
        );

        let jpeg = [
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x03, 0x00, 0x02, 0x03, 0x01, 0x11,
            0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00,
        ];
        assert_eq!(
            detect_jpeg(&jpeg),
            Some(BlossomImageDimensions::new(2, 3).unwrap())
        );

        let mut webp = vec![0_u8; 30];
        webp[..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        webp[12..16].copy_from_slice(b"VP8X");
        webp[24..27].copy_from_slice(&[1, 0, 0]);
        webp[27..30].copy_from_slice(&[2, 0, 0]);
        assert_eq!(
            detect_webp(&webp),
            Some(BlossomImageDimensions::new(2, 3).unwrap())
        );
    }

    #[test]
    fn image_header_decoders_reject_every_malformed_boundary() {
        let mut invalid_png_signature = vec![0_u8; 24];
        invalid_png_signature[12..16].copy_from_slice(b"IHDR");
        let mut invalid_png_chunk = png(2, 3);
        invalid_png_chunk[12..16].copy_from_slice(b"NOPE");
        let zero_png = png(0, 3);
        assert_eq!(detect_png(b"short"), None);
        assert_eq!(detect_png(&invalid_png_signature), None);
        assert_eq!(detect_png(&invalid_png_chunk), None);
        assert_eq!(detect_png(&zero_png), None);

        assert_eq!(detect_gif(b"short"), None);
        assert_eq!(detect_gif(b"GIF00a\x02\0\x03\0"), None);
        assert_eq!(
            detect_gif(b"GIF87a\x02\0\x03\0"),
            Some(BlossomImageDimensions::new(2, 3).unwrap())
        );
        assert_eq!(detect_gif(b"GIF89a\0\0\x03\0"), None);

        for malformed in [
            vec![0xff, 0xd8],
            vec![0, 0, 0, 0],
            vec![0xff, 0xd8, 0xff, 0xd9],
            vec![0xff, 0xd8, 0xff, 0xda],
            vec![0xff, 0xd8, 0x01],
            vec![0xff, 0xd8, 0xd0],
            vec![0xff, 0xd8, 0xff, 0xe0, 0, 1],
            vec![0xff, 0xd8, 0xff, 0xe0, 0, 100],
            vec![0xff, 0xd8, 0xff, 0xc0, 0, 6, 0, 0, 0, 0],
        ] {
            assert_eq!(detect_jpeg(&malformed), None, "{malformed:?}");
        }
        let jpeg_with_prefix = [
            0xff, 0xd8, 0xff, 0xe0, 0, 2, 0xff, 0xc2, 0, 7, 8, 0, 3, 0, 2,
        ];
        assert_eq!(
            detect_jpeg(&jpeg_with_prefix),
            Some(BlossomImageDimensions::new(2, 3).unwrap())
        );

        assert_eq!(detect_webp(b"short"), None);
        let mut wrong_riff = vec![0_u8; 30];
        wrong_riff[8..12].copy_from_slice(b"WEBP");
        assert_eq!(detect_webp(&wrong_riff), None);
        let mut wrong_webp = vec![0_u8; 30];
        wrong_webp[..4].copy_from_slice(b"RIFF");
        assert_eq!(detect_webp(&wrong_webp), None);

        let mut lossless = vec![0_u8; 30];
        lossless[..4].copy_from_slice(b"RIFF");
        lossless[8..12].copy_from_slice(b"WEBP");
        lossless[12..16].copy_from_slice(b"VP8L");
        lossless[20] = 0x2f;
        let packed = 1_u32 | (2_u32 << 14);
        lossless[21..25].copy_from_slice(&packed.to_le_bytes());
        assert_eq!(
            detect_webp(&lossless),
            Some(BlossomImageDimensions::new(2, 3).unwrap())
        );
        lossless[20] = 0;
        assert_eq!(detect_webp(&lossless), None);

        let mut lossy = vec![0_u8; 30];
        lossy[..4].copy_from_slice(b"RIFF");
        lossy[8..12].copy_from_slice(b"WEBP");
        lossy[12..16].copy_from_slice(b"VP8 ");
        lossy[23..26].copy_from_slice(&[0x9d, 0x01, 0x2a]);
        lossy[26..28].copy_from_slice(&2_u16.to_le_bytes());
        lossy[28..30].copy_from_slice(&3_u16.to_le_bytes());
        assert_eq!(
            detect_webp(&lossy),
            Some(BlossomImageDimensions::new(2, 3).unwrap())
        );
        lossy[23] = 0;
        assert_eq!(detect_webp(&lossy), None);
        lossy[12..16].copy_from_slice(b"NOPE");
        assert_eq!(detect_webp(&lossy), None);

        assert_eq!(little_u24(&[]), None);
        assert_eq!(little_u24(&[1]), None);
        assert_eq!(little_u24(&[1, 2]), None);
        assert_eq!(little_u24(&[1, 2, 3]), Some(0x03_02_01));
    }

    #[test]
    fn image_verification_supports_every_declared_mime() {
        let gif = b"GIF89a\x02\0\x03\0";
        assert!(
            verify_image(
                gif,
                &MediaType::parse("image/gif").unwrap(),
                BlossomImageDimensions::new(2, 3).unwrap(),
            )
            .is_ok()
        );

        let jpeg = [0xff, 0xd8, 0xff, 0xc0, 0, 7, 8, 0, 3, 0, 2];
        assert!(
            verify_image(
                &jpeg,
                &MediaType::parse("image/jpeg").unwrap(),
                BlossomImageDimensions::new(2, 3).unwrap(),
            )
            .is_ok()
        );

        let mut webp = vec![0_u8; 30];
        webp[..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        webp[12..16].copy_from_slice(b"VP8X");
        webp[24..27].copy_from_slice(&[1, 0, 0]);
        webp[27..30].copy_from_slice(&[2, 0, 0]);
        assert!(
            verify_image(
                &webp,
                &MediaType::parse("image/webp").unwrap(),
                BlossomImageDimensions::new(2, 3).unwrap(),
            )
            .is_ok()
        );
        assert_eq!(
            verify_image(
                &webp,
                &MediaType::parse("application/octet-stream").unwrap(),
                BlossomImageDimensions::new(2, 3).unwrap(),
            )
            .expect_err("unsupported media type")
            .kind(),
            BlossomErrorKind::UnsupportedMediaType
        );
    }

    #[tokio::test]
    async fn retry_and_error_classification_are_bounded_and_redacted() {
        for status in [
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::TOO_EARLY,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            assert!(http_status_error(status, BlossomPhase::Upload, true, 1).retryable());
        }
        assert!(
            !http_status_error(StatusCode::BAD_REQUEST, BlossomPhase::Upload, false, 1).retryable()
        );

        let cancellation = BlossomCancellation::default();
        assert!(ensure_not_cancelled(&cancellation, BlossomPhase::Upload, 0, false).is_ok());
        cancellation.cancel();
        assert_eq!(
            ensure_not_cancelled(&cancellation, BlossomPhase::Retrieval, 2, true)
                .expect_err("cancelled")
                .kind(),
            BlossomErrorKind::Cancelled
        );

        let profile = simulator_profile("http://127.0.0.1:9");
        let config = BlossomConfig::from_profile(profile)
            .with_network_policy(
                Duration::from_millis(10),
                Duration::from_millis(10),
                1,
                Duration::from_millis(1),
            )
            .unwrap();
        let delay = BlossomCancellation::default();
        assert!(
            retry_delay(&config, 1, &delay, BlossomPhase::Upload, false)
                .await
                .is_ok()
        );
        delay.cancel();
        assert_eq!(
            retry_delay(&config, 20, &delay, BlossomPhase::Retrieval, true,)
                .await
                .expect_err("cancelled delay")
                .kind(),
            BlossomErrorKind::Cancelled
        );

        let transport_error = reqwest::Client::new()
            .get("http://127.0.0.1:9")
            .send()
            .await
            .expect_err("closed port");
        assert_eq!(
            request_error(transport_error, BlossomPhase::Upload, false, 1).kind(),
            BlossomErrorKind::Transport
        );

        let bytes = png(2, 3);
        let request = upload_request("http://127.0.0.1:9", bytes.clone());
        let too_small = BlossomConfig::from_profile(simulator_profile("http://127.0.0.1:9"))
            .with_limits(1, 100, 0)
            .unwrap();
        assert_eq!(
            upload_with_authorization(
                too_small,
                request,
                "Nostr redacted",
                BlossomCancellation::default(),
            )
            .await
            .expect_err("oversize request")
            .kind(),
            BlossomErrorKind::ResponseTooLarge
        );
    }

    #[test]
    fn server_error_codes_are_bounded_validated_and_detail_free() {
        assert_eq!(
            parse_server_error_code(
                br#"{"error":"entitlement_missing","detail":"must never be retained"}"#
            )
            .as_deref(),
            Some("entitlement_missing")
        );
        assert_eq!(
            parse_server_error_code(br#"{"error":"unsafe-value"}"#),
            None
        );
        assert_eq!(parse_server_error_code(br#"{"error":"UPPERCASE"}"#), None);
        assert_eq!(
            parse_server_error_code(format!(r#"{{"error":"{}"}}"#, "a".repeat(65)).as_bytes()),
            None
        );
        assert_eq!(
            parse_server_error_code(vec![b' '; MAX_ERROR_RESPONSE_BYTES + 1].as_slice()),
            None
        );
        assert_eq!(parse_server_error_code(b"not-json"), None);
    }

    #[tokio::test]
    async fn http_failure_retains_only_the_public_server_error_code() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let request = read_request(&mut stream).await;
            assert!(String::from_utf8_lossy(&request).starts_with("PUT /upload HTTP/1.1"));
            let body =
                br#"{"error":"entitlement_missing","detail":"sensitive operational context"}"#;
            let response = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.expect("head");
            stream.write_all(body).await.expect("body");
            stream.shutdown().await.expect("close");
        });
        let origin = format!("http://{address}");
        let config = config(origin.as_str());
        let endpoint = config.profile().primary().clone();
        let error = upload_once(
            &config,
            &endpoint,
            &upload_request(origin.as_str(), png(2, 3)),
            "Nostr redacted",
            &BlossomCancellation::default(),
            1,
        )
        .await
        .expect_err("forbidden upload");
        assert_eq!(error.http_status(), Some(403));
        assert_eq!(error.server_error_code(), Some("entitlement_missing"));
        assert!(!format!("{error:?}").contains("sensitive operational context"));
        assert!(!format!("{error:?}").contains("Nostr redacted"));
        server.await.expect("server");
    }

    #[test]
    fn descriptor_verification_checks_each_identity_field() {
        let bytes = png(2, 3);
        let request = upload_request("http://127.0.0.1:3000", bytes);
        let expected_url =
            BlobUrl::parse(format!("http://127.0.0.1:3000/{}.png", request.sha256()).as_str())
                .unwrap();
        let descriptor = |url: BlobUrl, size: u64, media_type: &str| {
            BlobDescriptor::new(
                url,
                request.sha256(),
                size,
                MediaType::parse(media_type).unwrap(),
                1,
            )
            .unwrap()
        };
        assert!(
            verify_descriptor(
                &request,
                &expected_url,
                descriptor(
                    BlobUrl::parse(
                        format!("http://localhost:3000/{}.png", request.sha256()).as_str()
                    )
                    .unwrap(),
                    request.byte_size(),
                    "image/png",
                ),
                1,
            )
            .is_err()
        );
        assert!(
            verify_descriptor(
                &request,
                &expected_url,
                descriptor(expected_url.clone(), request.byte_size() + 1, "image/png"),
                1,
            )
            .is_err()
        );
        assert!(
            verify_descriptor(
                &request,
                &expected_url,
                descriptor(expected_url.clone(), request.byte_size(), "image/jpeg",),
                1,
            )
            .is_err()
        );
    }

    #[derive(Clone, Copy)]
    enum RetrievalResponse {
        Exact,
        Altered,
        RedirectExternal,
        WrongMime,
        Oversize,
        Stall,
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 1024];
            let read = stream.read(&mut chunk).await.expect("request read");
            assert_ne!(read, 0, "request ended before headers");
            request.extend_from_slice(&chunk[..read]);
            if let Some(end) = request.windows(4).position(|value| value == b"\r\n\r\n") {
                break end + 4;
            }
            assert!(request.len() < 64 * 1024, "request headers are bounded");
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length: ")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        while request.len() - header_end < content_length {
            let mut chunk = [0_u8; 1024];
            let read = stream.read(&mut chunk).await.expect("body read");
            assert_ne!(read, 0, "request ended before body");
            request.extend_from_slice(&chunk[..read]);
        }
        request
    }

    async fn spawn_server(
        bytes: Vec<u8>,
        retrieval: RetrievalResponse,
        bad_descriptor: bool,
    ) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let origin = format!("http://{address}");
        let expected_hash = Sha256::digest(bytes.as_slice());
        let descriptor_hash = if bad_descriptor {
            Sha256::digest(b"different")
        } else {
            expected_hash
        };
        let descriptor_url = format!("{origin}/{descriptor_hash}.png");
        let descriptor = BlobDescriptor::new(
            BlobUrl::parse(descriptor_url.as_str()).expect("url"),
            descriptor_hash,
            bytes.len() as u64,
            MediaType::parse("image/png").expect("media type"),
            1_900_000_000,
        )
        .expect("descriptor");
        let descriptor_json = serde_json::to_vec(&descriptor).expect("descriptor json");
        let task = tokio::spawn(async move {
            let (mut upload, _) = listener.accept().await.expect("upload accept");
            let upload_request = read_request(&mut upload).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                descriptor_json.len()
            );
            upload
                .write_all(response.as_bytes())
                .await
                .expect("upload head");
            upload
                .write_all(&descriptor_json)
                .await
                .expect("upload body");
            upload.shutdown().await.expect("upload close");

            if bad_descriptor {
                return upload_request;
            }
            let (mut retrieval_stream, _) = listener.accept().await.expect("retrieval accept");
            let _ = read_request(&mut retrieval_stream).await;
            match retrieval {
                RetrievalResponse::Exact | RetrievalResponse::Altered => {
                    let body = if matches!(retrieval, RetrievalResponse::Altered) {
                        let mut altered = bytes.clone();
                        altered[0] ^= 1;
                        altered
                    } else {
                        bytes
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    retrieval_stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("retrieval head");
                    retrieval_stream
                        .write_all(&body)
                        .await
                        .expect("retrieval body");
                }
                RetrievalResponse::RedirectExternal => {
                    let location = format!("https://example.com/{expected_hash}.png");
                    let response = format!(
                        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    retrieval_stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("redirect");
                }
                RetrievalResponse::WrongMime => {
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    );
                    retrieval_stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("wrong MIME");
                    retrieval_stream
                        .write_all(&bytes)
                        .await
                        .expect("retrieval body");
                }
                RetrievalResponse::Oversize => {
                    retrieval_stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 999999\r\nConnection: close\r\n\r\n")
                        .await
                        .expect("oversize");
                }
                RetrievalResponse::Stall => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
            retrieval_stream.shutdown().await.expect("retrieval close");
            upload_request
        });
        (origin, task)
    }

    async fn spawn_retry_server(bytes: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let origin = format!("http://{address}");
        let hash = Sha256::digest(bytes.as_slice());
        let descriptor = BlobDescriptor::new(
            BlobUrl::parse(format!("{origin}/{hash}.png").as_str()).expect("url"),
            hash,
            bytes.len() as u64,
            MediaType::parse("image/png").expect("media type"),
            1_900_000_000,
        )
        .expect("descriptor");
        let descriptor_json = serde_json::to_vec(&descriptor).expect("descriptor JSON");
        let task = tokio::spawn(async move {
            for step in 0..4 {
                let (mut stream, _) = listener.accept().await.expect("accept");
                let _ = read_request(&mut stream).await;
                match step {
                    0 => {
                        stream
                            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                            .await
                            .expect("upload retry");
                    }
                    1 => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            descriptor_json.len()
                        );
                        stream
                            .write_all(head.as_bytes())
                            .await
                            .expect("descriptor head");
                        stream
                            .write_all(&descriptor_json)
                            .await
                            .expect("descriptor body");
                    }
                    2 => {
                        stream
                            .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                            .await
                            .expect("retrieval retry");
                    }
                    3 => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            bytes.len()
                        );
                        stream
                            .write_all(head.as_bytes())
                            .await
                            .expect("retrieval head");
                        stream.write_all(&bytes).await.expect("retrieval body");
                    }
                    _ => unreachable!(),
                }
                stream.shutdown().await.expect("close");
            }
        });
        (origin, task)
    }

    fn upload_request(_origin: &str, bytes: Vec<u8>) -> BlossomUploadRequest {
        BlossomUploadRequest::new(
            Arc::from(bytes),
            MediaType::parse("image/png").expect("media type"),
            BlossomImageDimensions::new(2, 3).expect("dimensions"),
            1_900_000_000_000,
        )
        .expect("upload request")
    }

    fn config(origin: &str) -> BlossomConfig {
        BlossomConfig::from_profile(simulator_profile(origin))
            .with_network_policy(
                Duration::from_millis(100),
                Duration::from_millis(100),
                1,
                Duration::from_millis(1),
            )
            .expect("network policy")
    }

    fn simulator_profile(origin: &str) -> crate::transport::BlossomProfile {
        crate::transport::BlossomProfile::new(
            crate::transport::BlossomHostKind::Simulator,
            crate::transport::BlossomEndpointAuthority::LoopbackDevelopment,
            origin,
            std::iter::empty::<&str>(),
        )
        .expect("profile")
    }

    async fn spawn_inbound_server(
        bytes: Vec<u8>,
        content_type: &'static str,
        content_encoding: Option<&'static str>,
        declared_length: Option<usize>,
    ) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("address");
        let origin = format!("http://{address}");
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let request = read_request(&mut stream).await;
            let encoding = content_encoding
                .map(|value| format!("Content-Encoding: {value}\r\n"))
                .unwrap_or_default();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\n{encoding}Content-Length: {}\r\nConnection: close\r\n\r\n",
                declared_length.unwrap_or(bytes.len())
            );
            stream.write_all(response.as_bytes()).await.expect("head");
            stream.write_all(&bytes).await.expect("body");
            stream.shutdown().await.expect("close");
            request
        });
        (origin, task)
    }

    fn inbound_request(
        origin: &str,
        bytes: &[u8],
        dimensions: BlossomImageDimensions,
    ) -> BlossomInboundRequest {
        let hash = Sha256::digest(bytes);
        BlossomInboundRequest::new(
            BlobUrl::parse(format!("{origin}/{hash}.png").as_str()).expect("URL"),
            Some(MediaType::parse("image/png").expect("media type")),
            Some(bytes.len() as u64),
            Some(dimensions),
        )
        .expect("inbound request")
    }

    #[tokio::test]
    async fn inbound_retrieval_returns_only_exact_verified_image_bytes() {
        let bytes = png(2, 3);
        let (origin, server) = spawn_inbound_server(bytes.clone(), "image/png", None, None).await;
        let slot = crate::transport::BlossomSlot::new();
        slot.configure(config(origin.as_str())).expect("configure");
        let receipt = slot
            .retrieve(
                inbound_request(
                    origin.as_str(),
                    bytes.as_slice(),
                    BlossomImageDimensions::new(2, 3).unwrap(),
                ),
                BlossomCancellation::default(),
            )
            .await
            .expect("inbound receipt");
        assert_eq!(receipt.bytes(), bytes.as_slice());
        assert_eq!(receipt.commitment().sha256(), Sha256::digest(&bytes));
        assert_eq!(receipt.commitment().size(), bytes.len() as u64);
        assert_eq!(receipt.commitment().media_type().as_str(), "image/png");
        assert_eq!(
            receipt.dimensions(),
            BlossomImageDimensions::new(2, 3).unwrap()
        );
        assert_eq!(receipt.attempts(), 1);
        assert_eq!(
            receipt.config_fingerprint(),
            slot.config_fingerprint().expect("fingerprint")
        );
        let request = server.await.expect("server");
        let headers = String::from_utf8_lossy(&request);
        assert!(headers.starts_with("GET /"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("accept-encoding: identity")
        );
        assert!(!headers.to_ascii_lowercase().contains("authorization:"));
    }

    #[tokio::test]
    async fn inbound_retrieval_rejects_encoding_truncation_hash_and_dimensions() {
        let exact = png(2, 3);
        let cases = [
            (
                exact.clone(),
                Some("gzip"),
                None,
                BlossomImageDimensions::new(2, 3).unwrap(),
                BlossomErrorKind::ContentEncodingDenied,
            ),
            (
                exact[..exact.len() - 1].to_vec(),
                None,
                Some(exact.len()),
                BlossomImageDimensions::new(2, 3).unwrap(),
                BlossomErrorKind::Transport,
            ),
            (
                {
                    let mut altered = exact.clone();
                    let last = altered.len() - 1;
                    altered[last] ^= 1;
                    altered
                },
                None,
                None,
                BlossomImageDimensions::new(2, 3).unwrap(),
                BlossomErrorKind::ResponseHashMismatch,
            ),
            (
                exact.clone(),
                None,
                None,
                BlossomImageDimensions::new(3, 2).unwrap(),
                BlossomErrorKind::DimensionMismatch,
            ),
        ];
        for (served, encoding, length, dimensions, expected) in cases {
            let (origin, server) =
                spawn_inbound_server(served, "image/png", encoding, length).await;
            let slot = crate::transport::BlossomSlot::new();
            slot.configure(config(origin.as_str())).expect("configure");
            let error = slot
                .retrieve(
                    inbound_request(origin.as_str(), exact.as_slice(), dimensions),
                    BlossomCancellation::default(),
                )
                .await
                .expect_err("hostile response");
            assert_eq!(error.kind(), expected);
            assert!(!error.possible_orphan());
            server.await.expect("server");
        }
    }

    #[tokio::test]
    async fn inbound_retry_and_cancellation_are_bounded_and_operation_scoped() {
        let bytes = png(2, 3);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let origin = format!("http://{}", listener.local_addr().expect("address"));
        let server_bytes = bytes.clone();
        let server = tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().await.expect("accept");
                let _ = read_request(&mut stream).await;
                if attempt == 0 {
                    stream
                        .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .await
                        .expect("retry response");
                } else {
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        server_bytes.len()
                    );
                    stream.write_all(response.as_bytes()).await.expect("head");
                    stream.write_all(&server_bytes).await.expect("body");
                }
                stream.shutdown().await.expect("close");
            }
        });
        let slot = crate::transport::BlossomSlot::new();
        slot.configure(
            BlossomConfig::from_profile(simulator_profile(origin.as_str()))
                .with_network_policy(
                    Duration::from_millis(100),
                    Duration::from_millis(100),
                    2,
                    Duration::from_millis(1),
                )
                .unwrap(),
        )
        .unwrap();
        let receipt = slot
            .retrieve(
                inbound_request(
                    origin.as_str(),
                    bytes.as_slice(),
                    BlossomImageDimensions::new(2, 3).unwrap(),
                ),
                BlossomCancellation::default(),
            )
            .await
            .expect("retried retrieval");
        assert_eq!(receipt.attempts(), 2);
        server.await.expect("server");

        let cancellation = BlossomCancellation::default();
        cancellation.cancel();
        let error = slot
            .retrieve(
                inbound_request(
                    origin.as_str(),
                    bytes.as_slice(),
                    BlossomImageDimensions::new(2, 3).unwrap(),
                ),
                cancellation,
            )
            .await
            .expect_err("cancelled retrieval");
        assert_eq!(error.kind(), BlossomErrorKind::Cancelled);
        assert_eq!(error.phase(), BlossomPhase::Retrieval);
        assert!(!error.possible_orphan());
    }

    #[tokio::test]
    async fn non_mutating_probe_records_only_dns_transport_and_http_evidence() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 2_048];
            let count = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request.starts_with(&format!("GET /{BLOSSOM_PROBE_HASH} HTTP/1.1")));
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
            stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });
        let slot = crate::transport::BlossomSlot::new();
        slot.configure(config(format!("http://{address}").as_str()))
            .unwrap();
        let initial = slot.evidence().unwrap();
        assert_eq!(
            initial.state(),
            crate::transport::BlossomEvidenceState::ConfiguredUnobserved
        );
        assert!(initial.observed_at_unix_ms().is_none());

        let observed = slot.probe(BlossomCancellation::default()).await.unwrap();
        assert_eq!(
            observed.state(),
            crate::transport::BlossomEvidenceState::TlsHttpObserved
        );
        assert_eq!(observed.http_status(), Some(404));
        assert!(observed.error_code().is_none());
        assert!(observed.observed_at_unix_ms().is_some());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn cancelled_probe_is_retryable_redacted_and_never_claims_dns() {
        let slot = crate::transport::BlossomSlot::new();
        slot.configure(config("http://127.0.0.1:9")).unwrap();
        let cancellation = BlossomCancellation::default();
        cancellation.cancel();
        let error = slot.probe(cancellation).await.unwrap_err();
        assert_eq!(error.kind(), BlossomErrorKind::Cancelled);
        let evidence = slot.evidence().unwrap();
        assert_eq!(
            evidence.state(),
            crate::transport::BlossomEvidenceState::RetryableFailure
        );
        assert_eq!(
            evidence.last_successful_state(),
            crate::transport::BlossomEvidenceState::ConfiguredUnobserved
        );
        assert_eq!(evidence.error_code(), Some("blossom_cancelled"));
        assert_eq!(evidence.error_phase(), Some(BlossomPhase::Probe));
        assert!(!evidence.possible_orphan());
    }

    #[tokio::test]
    async fn reconfiguration_during_probe_cannot_promote_stale_evidence() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 2_048];
            let _ = stream.read(&mut request).await.unwrap();
            accepted_tx.send(()).unwrap();
            release_rx.await.unwrap();
            stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });
        let slot = crate::transport::BlossomSlot::new();
        slot.configure(config(format!("http://{address}").as_str()))
            .unwrap();
        let probing = {
            let slot = slot.clone();
            tokio::spawn(async move { slot.probe(BlossomCancellation::default()).await })
        };
        accepted_rx.await.unwrap();
        slot.configure(config("http://127.0.0.1:9")).unwrap();
        release_tx.send(()).unwrap();
        let error = probing.await.unwrap().unwrap_err();
        assert_eq!(error.kind(), BlossomErrorKind::ConfigurationChanged);
        assert_eq!(error.phase(), BlossomPhase::Probe);
        assert!(!error.possible_orphan());
        let evidence = slot.evidence().unwrap();
        assert_eq!(evidence.origin(), "http://127.0.0.1:9");
        assert_eq!(
            evidence.state(),
            crate::transport::BlossomEvidenceState::ConfiguredUnobserved
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn loopback_upload_preserves_exact_bytes_and_verifies_retrieval() {
        let bytes = png(2, 3);
        let (origin, server) = spawn_server(bytes.clone(), RetrievalResponse::Exact, false).await;
        let receipt = upload_with_authorization(
            config(origin.as_str()),
            upload_request(origin.as_str(), bytes.clone()),
            "Nostr secret-token-value",
            BlossomCancellation::default(),
        )
        .await
        .expect("verified upload");
        assert_eq!(receipt.descriptor().size(), bytes.len() as u64);
        assert_eq!(
            receipt.dimensions(),
            BlossomImageDimensions::new(2, 3).unwrap()
        );
        let upload_wire = server.await.expect("server");
        let header_end = upload_wire
            .windows(4)
            .position(|value| value == b"\r\n\r\n")
            .unwrap()
            + 4;
        assert_eq!(&upload_wire[header_end..], bytes.as_slice());
        let headers = String::from_utf8_lossy(&upload_wire[..header_end]);
        assert!(headers.contains("authorization: Nostr secret-token-value"));
        assert!(!format!("{receipt:?}").contains("secret-token-value"));
    }

    #[tokio::test]
    async fn retryable_upload_and_retrieval_failures_recover_with_bounded_attempts() {
        let bytes = png(2, 3);
        let (origin, server) = spawn_retry_server(bytes.clone()).await;
        let config = BlossomConfig::from_profile(simulator_profile(origin.as_str()))
            .with_network_policy(
                Duration::from_millis(100),
                Duration::from_millis(100),
                2,
                Duration::from_millis(1),
            )
            .expect("network policy");
        let receipt = upload_with_authorization(
            config,
            upload_request(origin.as_str(), bytes),
            "Nostr redacted",
            BlossomCancellation::default(),
        )
        .await
        .expect("retry recovery");
        assert_eq!(receipt.attempts(), 4);
        server.await.expect("server");
    }

    #[tokio::test]
    async fn descriptor_redirect_body_mime_timeout_and_cancellation_fail_closed() {
        let cases = [
            (
                RetrievalResponse::Exact,
                true,
                BlossomErrorKind::DescriptorMismatch,
            ),
            (
                RetrievalResponse::RedirectExternal,
                false,
                BlossomErrorKind::UnsafeRedirect,
            ),
            (
                RetrievalResponse::Altered,
                false,
                BlossomErrorKind::ResponseHashMismatch,
            ),
            (
                RetrievalResponse::WrongMime,
                false,
                BlossomErrorKind::MediaTypeMismatch,
            ),
            (
                RetrievalResponse::Oversize,
                false,
                BlossomErrorKind::ResponseSizeMismatch,
            ),
            (RetrievalResponse::Stall, false, BlossomErrorKind::Timeout),
        ];
        for (response, bad_descriptor, expected) in cases {
            let bytes = png(2, 3);
            let (origin, server) = spawn_server(bytes.clone(), response, bad_descriptor).await;
            let error = upload_with_authorization(
                config(origin.as_str()),
                upload_request(origin.as_str(), bytes),
                "Nostr redacted",
                BlossomCancellation::default(),
            )
            .await
            .expect_err("must fail closed");
            assert_eq!(error.kind(), expected);
            assert!(error.possible_orphan());
            assert!(!format!("{error:?}").contains("Nostr redacted"));
            server.await.expect("server");
        }

        let cancellation = BlossomCancellation::default();
        cancellation.cancel();
        let bytes = png(2, 3);
        let request = BlossomUploadRequest::new(
            Arc::from(bytes),
            MediaType::parse("image/png").unwrap(),
            BlossomImageDimensions::new(2, 3).unwrap(),
            1,
        )
        .unwrap();
        let error = upload_with_authorization(
            config("http://127.0.0.1:9"),
            request,
            "Nostr redacted",
            cancellation,
        )
        .await
        .expect_err("cancelled");
        assert_eq!(error.kind(), BlossomErrorKind::Cancelled);
        assert!(!error.possible_orphan());
    }
}
