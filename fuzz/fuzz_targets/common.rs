use radroots_blossom::{
    AuthoredRasterDimensions, BlobDescriptor, BlobUrl, Bud01GetObservation, Bud01HeadObservation,
    Bud02UploadObservation, MediaType, RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES, Sha256,
    verify_publication_readiness,
};
pub(crate) fn exercise(input: &[u8], media_type: &str, extension: &str) {
    if input.is_empty() || input.len() as u64 > RADROOTS_BLOSSOM_PUBLICATION_RASTER_MAX_BYTES {
        return;
    }

    let hash = Sha256::digest(input);
    let url = format!("https://cdn.example/{hash}.{extension}");
    let Ok(media_type) = MediaType::parse(media_type) else {
        return;
    };
    let Ok(url) = BlobUrl::parse(&url) else {
        return;
    };
    let Ok(descriptor) = BlobDescriptor::new(
        url.clone(),
        hash,
        input.len() as u64,
        media_type.clone(),
        1_800_000_000,
    ) else {
        return;
    };
    let Ok(authored) = descriptor
        .clone()
        .approve_reference()
        .and_then(|descriptor| descriptor.verify_bytes(input, &media_type))
    else {
        return;
    };
    let Ok(upload) = Bud02UploadObservation::new(201, descriptor) else {
        return;
    };
    let Ok(approved_url) = url.approve() else {
        return;
    };
    let Ok(head) = Bud01HeadObservation::new(
        200,
        approved_url.clone(),
        input.len() as u64,
        media_type,
    ) else {
        return;
    };
    let Ok(get) = Bud01GetObservation::from_complete_body(
        200,
        approved_url,
        input.len() as u64,
        input,
    ) else {
        return;
    };

    let _ = verify_publication_readiness(
        &authored,
        input,
        AuthoredRasterDimensions::Unspecified,
        &upload,
        &head,
        &get,
    );
}
