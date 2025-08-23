use nostr::{event::Kind, filter::Filter, types::Timestamp};

pub fn nostr_kind(kind: u16) -> Kind {
    Kind::Custom(kind)
}

pub fn nostr_filter_kind(kind: u16) -> Filter {
    Filter::new().kind(Kind::Custom(kind))
}

pub fn nostr_filter_new_events(filter: Filter) -> Filter {
    filter.since(Timestamp::now())
}
