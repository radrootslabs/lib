use super::*;

#[tokio::test]
async fn monetary_listings_sign_install_deliver_and_reopen_without_repricing_or_new_issuance() {
    let mut relay = Relay::new().await;
    let ready = Ready::new(&relay, StartMode::Online, PublicSigner::new(), true).await;
    let mut expected = Vec::new();
    for (id, price, contents, expected_price) in [
        (
            240,
            json!({"type":"fixed","rate":{"currency":"CAD","minor":"325","exponent":2},
            "basis":"2","discount_bps":1000,"tiers":[{"from":"10","rate":{"currency":"CAD","minor":"299","exponent":2}}]}),
            json!({"type":"fixed","quantity":{"amount":"0.5","unit":"kg"}}),
            Some("3.25"),
        ),
        (
            241,
            json!({"type":"variable_weight","measurement_unit":"kg","minimum_per_sale_unit":"0.8",
            "maximum_per_sale_unit":"1.2","rate":{"currency":"CAD","minor":"325","exponent":2},
            "basis":"1","maximum_per_sale_unit_total":{"currency":"CAD","minor":"400","exponent":2}}),
            json!({"type":"estimated","quantity":{"amount":"1","unit":"kg"}}),
            None,
        ),
    ] {
        let mut wire = serde_json::to_value(input(id)).unwrap();
        wire["summary"] = json!("Native carrot offering");
        wire["profile"]["media"] = json!([{"url":"https://media.fixture.invalid/carrot",
            "sha256":"ab".repeat(32),"mime":"image/jpeg","alt":"Carrot 🥕"},
            {"url":"ipfs://declared-object","sha256":"00".repeat(32),
            "mime":"application/octet-stream","alt":""}]);
        wire["profile"]["options"][0]["price"] = price;
        wire["profile"]["options"][0]["unit"]["contents"] = contents;
        let captured = PublishInput::parse(&serde_json::to_vec(&wire).unwrap()).unwrap();
        let deadline = now() + 120;
        let receipt = ready
            .client
            .listings()
            .publish(ready.context(id, deadline), captured.clone())
            .await
            .unwrap();
        assert_eq!(
            receipt.business_installation(),
            BusinessInstallation::Installed
        );
        assert_eq!(receipt.issuance(), Issuance::Obtained);
        assert_eq!(receipt.delivery(), Delivery::Observed);
        let listing = receipt.business_result().unwrap().clone();
        assert_eq!(listing.terms(), captured.terms());
        let raw = tokio::time::timeout(Duration::from_secs(5), relay.events.recv())
            .await
            .unwrap()
            .unwrap();
        let independent = nostr::Event::from_json(&raw).unwrap();
        independent.verify().unwrap();
        assert_eq!(independent.kind.as_u16(), 30402);
        assert_eq!(independent.id.as_bytes(), listing.revision().as_bytes());
        let prices: Vec<_> = independent
            .tags
            .iter()
            .filter(|tag| tag.as_slice()[0] == "price")
            .collect();
        match expected_price {
            Some(amount) => {
                assert_eq!(prices.len(), 1);
                assert_eq!(prices[0].as_slice(), ["price", amount, "CAD"]);
                assert_eq!(independent.tags.len(), 9);
            }
            None => assert!(prices.is_empty()),
        }
        let profile_tag = independent
            .tags
            .iter()
            .find(|tag| tag.as_slice()[0] == "radroots:listing")
            .unwrap();
        let actual: serde_json::Value = serde_json::from_str(&profile_tag.as_slice()[1]).unwrap();
        assert_eq!(actual, wire["profile"]);
        assert_eq!(listing.terms().media().len(), 2);
        let replay = ready
            .client
            .listings()
            .publish(ready.context(id, deadline), captured)
            .await
            .unwrap();
        assert_eq!(
            replay.operation_checkpoint(),
            receipt.operation_checkpoint()
        );
        expected.push((
            id,
            listing,
            receipt.command_digest(),
            receipt.operation_checkpoint(),
        ));
    }
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 2);
    assert_eq!(ready.signer.private_calls.load(Ordering::SeqCst), 0);
    assert_eq!(relay.requests.load(Ordering::SeqCst), 0);
    assert_eq!(relay.count.load(Ordering::SeqCst), 2);
    ready.client.close().await.unwrap();
    let store = ready.fixture.reopen(ready.input.clone()).await;
    let (client, scope) = open(
        &ready.fixture,
        &store,
        &relay,
        "cache-monetary-reopen",
        ready.signer.clone(),
        (&ready.input, &ready.proof),
        StartMode::Online,
    )
    .await;
    for (id, listing, digest, checkpoint) in expected {
        let retained = client
            .operations(&scope)
            .get(OperationId::from_bytes([id; 32]))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            retained.business_result().unwrap().as_listing().unwrap(),
            &listing
        );
        assert_eq!(retained.operation_checkpoint(), checkpoint);
        let resumed = client
            .operations(&scope)
            .resume(OperationId::from_bytes([id; 32]), Some(digest))
            .await
            .unwrap();
        assert_eq!(resumed.operation_checkpoint(), checkpoint);
    }
    assert_eq!(ready.signer.calls.load(Ordering::SeqCst), 2);
    assert_eq!(relay.count.load(Ordering::SeqCst), 2);
    client.close().await.unwrap();
    stop(relay).await;
}
