use radroots_blossom::{BlobDescriptor, BlobUrl, MediaType, Sha256};

fn main() -> Result<(), radroots_blossom::Error> {
    let bytes = b"hello";
    let hash = Sha256::digest(bytes);
    let media_type = MediaType::parse("text/plain")?;
    let url = BlobUrl::parse(&format!("https://media.example/{hash}.txt"))?;
    let descriptor = BlobDescriptor::new(
        url,
        hash,
        bytes.len() as u64,
        media_type.clone(),
        1_725_105_921,
    )?;

    let verified = descriptor
        .approve_reference()?
        .verify_bytes(bytes, &media_type)?;

    assert_eq!(verified.sha256(), hash);
    assert_eq!(verified.size(), 5);
    assert_eq!(
        verified.url().as_str(),
        format!("https://media.example/{hash}.txt")
    );
    Ok(())
}
