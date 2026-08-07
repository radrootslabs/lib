#[path = "../src/test_fixtures.rs"]
mod test_fixtures;

use nostr::{Keys, SecretKey, UnsignedEvent};
use radroots_event::calendar::{
    AuthoredCalendarDateEvent, AuthoredCalendarTimeEvent, CalendarDate,
};
use radroots_nostr::event::{Timestamp, build_calendar_date, build_calendar_time};

#[test]
fn typed_calendar_builders_finalize_exact_plans_for_an_external_host_signer() {
    let keys = Keys::new(
        SecretKey::from_hex(test_fixtures::FIXTURE_ALICE_SECRET_KEY_HEX).expect("fixture key"),
    );
    let created_at = Timestamp::from_secs(1_784_347_200);
    let date = AuthoredCalendarDateEvent::new(
        "market-day",
        "Saturday Market",
        CalendarDate::parse("2026-08-08").expect("date"),
    )
    .expect("date event");
    let time = AuthoredCalendarTimeEvent::new("farm-tour", "Farm Tour", 1_784_380_800)
        .expect("time event");

    let builders = [
        (
            "radroots.calendar.date_event.v1",
            31_922,
            build_calendar_date(&date).expect("date builder"),
        ),
        (
            "radroots.calendar.time_event.v1",
            31_923,
            build_calendar_time(&time).expect("time builder"),
        ),
    ];
    for (contract_id, kind, builder) in builders {
        let request = builder
            .custom_created_at(created_at)
            .into_external_signing_request(keys.public_key())
            .expect("external signing request");
        let plan = request.authored_plan().expect("typed plan").clone();
        assert_eq!(plan.body().contract().contract_id().as_str(), contract_id);
        assert_eq!(plan.body().kind(), kind);
        assert_eq!(plan.created_at(), created_at.as_secs());

        let unsigned: UnsignedEvent =
            serde_json::from_value(serde_json::to_value(&request).expect("request JSON"))
                .expect("unsigned event");
        let signed = unsigned.sign_with_keys(&keys).expect("host signature");
        let completed = request.complete(signed).expect("exact completion");
        assert_eq!(completed.id.to_hex(), plan.expected_event_id().to_hex());
    }
}
