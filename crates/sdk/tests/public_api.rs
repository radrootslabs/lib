use std::{any::type_name, collections::BTreeSet};

const ACTIVE_MODULES: &[(&str, &str)] = &[
    ("capability", include_str!("../src/capability.rs")),
    ("client", include_str!("../src/client.rs")),
    ("diagnostics", include_str!("../src/diagnostics.rs")),
    ("error", include_str!("../src/error.rs")),
    ("farm", include_str!("../src/farm.rs")),
    ("listing", include_str!("../src/listing.rs")),
    ("signing", include_str!("../src/signing.rs")),
    ("storage", include_str!("../src/storage.rs")),
    ("sync", include_str!("../src/sync.rs")),
    ("trade", include_str!("../src/trade.rs")),
    ("transport", include_str!("../src/transport.rs")),
];

#[test]
fn public_native_type_snapshot_uses_contextual_names() {
    let actual = BTreeSet::from([
        type_name::<radroots_sdk::Client>(),
        type_name::<radroots_sdk::ClientBuilder>(),
        type_name::<radroots_sdk::capability::Availability>(),
        type_name::<radroots_sdk::capability::CapabilityId>(),
        type_name::<radroots_sdk::capability::CapabilityReport>(),
        type_name::<radroots_sdk::capability::CapabilityStatus>(),
        type_name::<radroots_sdk::capability::Maturity>(),
        type_name::<radroots_sdk::diagnostics::Report>(),
        type_name::<radroots_sdk::error::Error>(),
        type_name::<radroots_sdk::error::ErrorDescriptor>(),
        type_name::<radroots_sdk::error::ErrorKind>(),
        type_name::<radroots_sdk::farm::Plan>(),
        type_name::<radroots_sdk::farm::PrepareError>(),
        type_name::<radroots_sdk::farm::PrepareErrorKind>(),
        type_name::<radroots_sdk::farm::PrepareRequest>(),
        type_name::<radroots_sdk::listing::Action>(),
        type_name::<radroots_sdk::listing::Plan>(),
        type_name::<radroots_sdk::listing::PrepareError>(),
        type_name::<radroots_sdk::listing::PrepareErrorKind>(),
        type_name::<radroots_sdk::listing::PrepareRequest>(),
        type_name::<radroots_sdk::signing::Mode>(),
        type_name::<radroots_sdk::signing::Provider>(),
        type_name::<radroots_sdk::storage::Operations<'static>>(),
        type_name::<radroots_sdk::trade::Plan>(),
        type_name::<radroots_sdk::trade::PrepareError>(),
        type_name::<radroots_sdk::trade::PrepareErrorKind>(),
        type_name::<radroots_sdk::trade::PrepareRequest>(),
        type_name::<radroots_sdk::transport::Profile>(),
    ]);
    #[cfg(feature = "sync")]
    let actual = actual
        .into_iter()
        .chain([
            type_name::<radroots_sdk::farm::EnqueueRequest>(),
            type_name::<radroots_sdk::farm::Operations<'static>>(),
            type_name::<radroots_sdk::listing::EnqueueRequest>(),
            type_name::<radroots_sdk::listing::Operations<'static>>(),
            type_name::<radroots_sdk::sync::Operations<'static>>(),
            type_name::<radroots_sdk::trade::EnqueueRequest>(),
            type_name::<radroots_sdk::trade::Operations<'static>>(),
            type_name::<radroots_sdk::trade::PrivateTermsError>(),
        ])
        .collect::<BTreeSet<_>>();
    #[cfg(feature = "radrootsd")]
    let actual = actual
        .into_iter()
        .chain([
            type_name::<radroots_sdk::transport::DaemonAuth>(),
            type_name::<radroots_sdk::transport::DaemonConfig>(),
            type_name::<radroots_sdk::transport::DaemonDelivery>(),
            type_name::<radroots_sdk::transport::DaemonError>(),
            type_name::<radroots_sdk::transport::DaemonErrorKind>(),
        ])
        .collect::<BTreeSet<_>>();

    assert!(actual.iter().all(|name| !name.contains("RadrootsSdk")));
    assert!(actual.iter().all(|name| {
        name.rsplit("::")
            .next()
            .is_some_and(|item| !item.starts_with("Sdk"))
    }));
    assert_eq!(actual.len(), expected_public_type_count());
}

#[test]
fn active_native_api_has_no_sdk_owned_traits_or_public_field_layout() {
    for (module, source) in ACTIVE_MODULES {
        for line in source.lines() {
            let line = line.trim_start();
            assert!(
                !line.starts_with("pub trait "),
                "{module} must reuse lower host SPIs instead of exposing an SDK-owned trait"
            );
            for declaration in ["pub struct ", "pub enum ", "pub trait ", "pub type "] {
                if let Some(item) = line.strip_prefix(declaration) {
                    let item = item
                        .split(|character: char| {
                            character == '<'
                                || character == '('
                                || character == '{'
                                || character == ';'
                                || character.is_whitespace()
                        })
                        .next()
                        .expect("public item name");
                    assert!(
                        !item.starts_with("RadrootsSdk") && !item.starts_with("Sdk"),
                        "{module} exposes representation-prefixed item {item}"
                    );
                }
            }

            let public_field = line.starts_with("pub ")
                && line.contains(':')
                && ![
                    "pub async fn ",
                    "pub const ",
                    "pub enum ",
                    "pub fn ",
                    "pub mod ",
                    "pub static ",
                    "pub struct ",
                    "pub trait ",
                    "pub type ",
                    "pub use ",
                ]
                .iter()
                .any(|prefix| line.starts_with(prefix));
            assert!(!public_field, "{module} exposes native field `{line}`");
        }
    }
}

const fn expected_public_type_count() -> usize {
    let count = 28;
    #[cfg(feature = "sync")]
    let count = count + 8;
    #[cfg(feature = "radrootsd")]
    let count = count + 5;
    count
}
