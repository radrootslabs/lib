#![cfg(all(
    feature = "identity-models",
    feature = "relay-client",
    feature = "signing"
))]

use futures::{SinkExt, StreamExt};
use nostr::{ClientMessage, JsonUtil, RelayMessage};
use radroots_core::{
    RadrootsCoreCurrency, RadrootsCoreDecimal, RadrootsCoreMoney, RadrootsCoreQuantity,
    RadrootsCoreQuantityPrice, RadrootsCoreUnit,
};
use radroots_sdk::identity::RadrootsIdentity;
use radroots_sdk::listing::{
    RadrootsListing, RadrootsListingAvailability, RadrootsListingBin,
    RadrootsListingDeliveryMethod, RadrootsListingFarmRef, RadrootsListingLocation,
    RadrootsListingProduct, RadrootsListingStatus,
};
use radroots_sdk::{
    RadrootsSdkClient, RadrootsSdkConfig, RelayConfig, SdkEnvironment, SdkPublishError,
    SdkTransportMode, SdkTransportReceipt, SignerConfig,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::Message;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct AckRelay {
    url: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl AckRelay {
    async fn spawn() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let url = format!("ws://{addr}");
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accept = listener.accept() => {
                        let Ok((stream, _)) = accept else {
                            break;
                        };
                        tokio::spawn(async move {
                            let Ok(websocket) = tokio_tungstenite::accept_async(stream).await else {
                                return;
                            };
                            let (mut writer, mut reader) = websocket.split();
                            while let Some(message) = reader.next().await {
                                let Ok(message) = message else {
                                    break;
                                };
                                let Message::Text(text) = message else {
                                    continue;
                                };
                                let Ok(client_message) = ClientMessage::from_json(text.as_str()) else {
                                    continue;
                                };
                                if let ClientMessage::Event(event) = client_message {
                                    let relay_message =
                                        RelayMessage::ok(event.id, true, "").as_json();
                                    if writer
                                        .send(Message::Text(relay_message.into()))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        });
                    }
                }
            }
        });

        Ok(Self {
            url,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    fn url(&self) -> &str {
        self.url.as_str()
    }
}

impl Drop for AckRelay {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

fn sample_listing() -> RadrootsListing {
    RadrootsListing {
        d_tag: "AAAAAAAAAAAAAAAAAAAAAg".into(),
        farm: RadrootsListingFarmRef {
            pubkey: "seller".into(),
            d_tag: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        },
        product: RadrootsListingProduct {
            key: "coffee".into(),
            title: "Coffee".into(),
            category: "coffee".into(),
            summary: Some("Single origin coffee".into()),
            process: None,
            lot: None,
            location: None,
            profile: None,
            year: None,
        },
        primary_bin_id: "bin-1".into(),
        bins: vec![RadrootsListingBin {
            bin_id: "bin-1".into(),
            quantity: RadrootsCoreQuantity::new(
                RadrootsCoreDecimal::from(1000u32),
                RadrootsCoreUnit::MassG,
            ),
            price_per_canonical_unit: RadrootsCoreQuantityPrice {
                amount: RadrootsCoreMoney::new(
                    RadrootsCoreDecimal::from(20u32),
                    RadrootsCoreCurrency::USD,
                ),
                quantity: RadrootsCoreQuantity::new(
                    RadrootsCoreDecimal::from(1u32),
                    RadrootsCoreUnit::MassG,
                ),
            },
            display_amount: None,
            display_unit: None,
            display_label: None,
            display_price: None,
            display_price_unit: None,
        }],
        resource_area: None,
        plot: None,
        discounts: None,
        inventory_available: Some(RadrootsCoreDecimal::from(5u32)),
        availability: Some(RadrootsListingAvailability::Status {
            status: RadrootsListingStatus::Active,
        }),
        delivery_method: Some(RadrootsListingDeliveryMethod::Pickup),
        location: Some(RadrootsListingLocation {
            primary: "North Farm".into(),
            city: None,
            region: None,
            country: None,
            lat: None,
            lng: None,
            geohash: None,
        }),
        images: None,
    }
}

#[tokio::test]
async fn relay_direct_listing_publish_accepts_sdk_built_draft() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::LocalIdentity;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;
    let draft = client.listing().build_draft(&sample_listing())?;

    let receipt = client
        .listing()
        .publish_draft_with_identity(&identity, draft)
        .await?;

    assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
    assert_eq!(receipt.event_kind, Some(30402));
    assert!(receipt.event_id.is_some());
    match receipt.transport_receipt {
        SdkTransportReceipt::RelayDirect(relay_receipt) => {
            assert_eq!(
                relay_receipt.acknowledged_relays,
                vec![relay.url().to_owned()]
            );
            assert!(relay_receipt.failed_relays.is_empty());
        }
        SdkTransportReceipt::Radrootsd(_) => panic!("unexpected radrootsd receipt"),
    }

    Ok(())
}

#[tokio::test]
async fn relay_direct_publish_rejects_radrootsd_transport_mode() -> TestResult<()> {
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::production();
    config.transport = SdkTransportMode::Radrootsd;
    config.signer = SignerConfig::LocalIdentity;
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .listing()
        .publish_with_identity(&identity, &sample_listing())
        .await
        .expect_err("unsupported transport");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedTransport {
            transport: SdkTransportMode::Radrootsd,
            operation: "listing.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_publish_rejects_draft_only_signer_mode() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::DraftOnly;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .listing()
        .publish_with_identity(&identity, &sample_listing())
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::DraftOnly,
            required: SignerConfig::LocalIdentity,
            operation: "listing.publish_with_identity",
        }
    ));

    Ok(())
}

#[tokio::test]
async fn relay_direct_publish_rejects_nip46_signer_mode() -> TestResult<()> {
    let relay = AckRelay::spawn().await?;
    let identity = RadrootsIdentity::generate();
    let mut config = RadrootsSdkConfig::for_environment(SdkEnvironment::Custom);
    config.transport = SdkTransportMode::RelayDirect;
    config.signer = SignerConfig::Nip46;
    config.relay = RelayConfig {
        urls: vec![relay.url().to_owned()],
    };
    let client = RadrootsSdkClient::from_config(config)?;

    let error = client
        .listing()
        .publish_with_identity(&identity, &sample_listing())
        .await
        .expect_err("unsupported signer mode");

    assert!(matches!(
        error,
        SdkPublishError::UnsupportedSignerMode {
            transport: SdkTransportMode::RelayDirect,
            signer: SignerConfig::Nip46,
            required: SignerConfig::LocalIdentity,
            operation: "listing.publish_with_identity",
        }
    ));

    Ok(())
}
