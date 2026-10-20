use crate::{check, pause};
use futures::FutureExt;
use radroots_transport::{
    DeliveryRequest, EventSink, EventSource, FetchRequest, TargetSet,
    outcome::FetchTargetState,
    policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
    sink::DeliveryPayload,
    source::FetchBounds,
};
use radroots_transport_nostr::{
    Config, RelayAccess, RelayEndpoint, RelayProfile, RelayProfileKind, RelayUrlPolicy,
};
use wasm_bindgen::prelude::*;

fn now() -> u64 {
    js_sys::Date::now() as u64
}
fn profile_config(base: &str, route: &str) -> Config {
    Config::from_profile(
        RelayProfile::explicit(
            RelayProfileKind::Simulator,
            [RelayEndpoint::new(
                format!("{base}/{route}"),
                RelayUrlPolicy::Local,
                RelayAccess::ReadWrite,
            )
            .unwrap()],
        )
        .unwrap(),
    )
    .with_timeouts(1000, 500, 100)
    .unwrap()
}
fn targets(config: &Config) -> TargetSet {
    TargetSet::new(
        config
            .relays()
            .iter()
            .map(|relay| relay.to_target().unwrap())
            .collect(),
    )
    .unwrap()
}

#[wasm_bindgen]
pub fn fixture_event() -> String {
    // Consume the source-owned conformance vector, retaining an independently
    // verified signature while deliberately changing only its outer whitespace.
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../contracts/conformance/vectors/event/verified_admission.v1.json"
    ))
    .unwrap();
    format!(
        " \n{} \t",
        serde_json::to_string_pretty(&vectors["vectors"][1]["input"]["event"]).unwrap()
    )
}

#[wasm_bindgen]
pub async fn verify_shared_session(base: String) -> Result<js_sys::Array, JsValue> {
    let passed = js_sys::Array::new();
    let raw = fixture_event();
    let config = profile_config(&base, "shared-accept");
    let transport = radroots_transport_nostr_browser::create_transport(config.clone());
    let page = transport
        .fetch(
            FetchRequest::new(
                "browser-source",
                targets(&config),
                FetchBounds::new(10, now() + 5000).unwrap(),
            )
            .unwrap(),
        )
        .await
        .map_err(|_| JsValue::from_str("shared fetch error"))?;
    check(
        page.events().len() == 1 && page.target_outcomes()[0].state() == FetchTargetState::Complete,
        &format!(
            "shared verified source and EOSE: events={} targets={:?}",
            page.events().len(),
            page.target_outcomes()
        ),
    )?;
    passed.push(&"shared independently verified event and EOSE".into());
    let request = DeliveryRequest::new(
        "browser-exact",
        DeliveryPayload::new(radroots_event_codec::decode::signed_event(&raw).unwrap()),
        targets(&config),
        SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
        now() + 5000,
    )
    .unwrap();
    let receipt = transport
        .deliver(request)
        .await
        .map_err(|_| JsValue::from_str("shared deliver error"))?;
    check(
        receipt.target_receipts()[0]
            .outcome()
            .satisfies(SatisfactionClass::Accepted),
        "shared exact acceptance",
    )?;
    transport.shutdown().await;
    check(transport.admission_is_closed(), "shared shutdown admission")?;
    passed.push(&"shared original publication and matching ACK".into());

    for route in ["shared-negative", "shared-noack", "shared-badack"] {
        let config = profile_config(&base, route);
        let transport = radroots_transport_nostr_browser::create_transport(config.clone());
        let request = DeliveryRequest::new(
            route,
            DeliveryPayload::new(radroots_event_codec::decode::signed_event(&raw).unwrap()),
            targets(&config),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all()),
            now() + 5000,
        )
        .unwrap();
        let receipt = transport
            .deliver(request)
            .await
            .map_err(|_| JsValue::from_str("shared negative deliver error"))?;
        check(
            !receipt.target_receipts()[0]
                .outcome()
                .satisfies(SatisfactionClass::Accepted),
            route,
        )?;
        transport.shutdown().await;
        passed.push(&JsValue::from_str(route));
    }

    let config = profile_config(&base, "shared-disconnect");
    let transport = radroots_transport_nostr_browser::create_transport(config.clone());
    let page = transport
        .fetch(
            FetchRequest::new(
                "browser-gap",
                targets(&config),
                FetchBounds::new(10, now() + 5000).unwrap(),
            )
            .unwrap(),
        )
        .await
        .map_err(|_| JsValue::from_str("shared disconnect fetch error"))?;
    check(
        page.target_outcomes()[0].state() != FetchTargetState::Complete,
        "disconnect cannot prove complete history",
    )?;
    transport.shutdown().await;
    passed.push(&"shared disconnect never proves complete history".into());

    let config = profile_config(&base, "shared-observer");
    let transport = radroots_transport_nostr_browser::create_transport(config.clone());
    let mut fetch = transport.fetch(
        FetchRequest::new(
            "browser-observer",
            targets(&config),
            FetchBounds::new(10, now() + 5000).unwrap(),
        )
        .unwrap(),
    );
    check(
        fetch.as_mut().now_or_never().is_none(),
        "shared fetch starts pending",
    )?;
    pause().await;
    check(
        fetch.as_mut().now_or_never().is_none(),
        "shared original observer remains pending",
    )?;
    drop(fetch);
    transport.shutdown().await;
    pause().await;
    check(
        !transport.has_in_flight_operations(),
        "shared observer loss frees original call",
    )?;
    passed.push(&"shared fetch observer loss and shutdown".into());
    Ok(passed)
}
