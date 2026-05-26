create table local_event_projection_cursor_previous (
  consumer_id text primary key,
  last_seq integer not null,
  updated_at_ms integer not null,
  check (trim(consumer_id) <> ''),
  check (last_seq >= 0)
);

insert into local_event_projection_cursor_previous(
  consumer_id,
  last_seq,
  updated_at_ms
)
select
  consumer_id,
  last_change_seq,
  updated_at_ms
from local_event_projection_cursor;

drop table local_event_projection_cursor;
alter table local_event_projection_cursor_previous rename to local_event_projection_cursor;

create table local_event_record_previous (
  seq integer primary key autoincrement,
  record_id text not null unique,
  family text not null check (family in ('local_work', 'signed_event')),
  status text not null check (status in ('local_draft', 'local_saved', 'pending_publish', 'published', 'failed', 'conflict')),
  source_runtime text not null check (source_runtime in ('cli', 'app', 'network', 'service', 'worker', 'test')),
  created_at_ms integer not null,
  inserted_at_ms integer not null,
  updated_at_ms integer not null,
  owner_account_id text,
  owner_pubkey text,
  farm_id text,
  listing_addr text,
  local_work_json text,
  event_id text,
  event_kind integer,
  event_pubkey text,
  event_created_at integer,
  event_tags_json text,
  event_content text,
  event_sig text,
  raw_event_json text,
  outbox_status text not null check (outbox_status in ('none', 'pending', 'acknowledged', 'failed')),
  relay_set_fingerprint text,
  relay_delivery_json text,
  check (trim(record_id) <> ''),
  check (family <> 'local_work' or local_work_json is not null),
  check (family <> 'local_work' or outbox_status = 'none'),
  check (family <> 'signed_event' or (event_id is not null and event_kind is not null and event_pubkey is not null and event_sig is not null and raw_event_json is not null))
);

insert into local_event_record_previous(
  seq,
  record_id,
  family,
  status,
  source_runtime,
  created_at_ms,
  inserted_at_ms,
  updated_at_ms,
  owner_account_id,
  owner_pubkey,
  farm_id,
  listing_addr,
  local_work_json,
  event_id,
  event_kind,
  event_pubkey,
  event_created_at,
  event_tags_json,
  event_content,
  event_sig,
  raw_event_json,
  outbox_status,
  relay_set_fingerprint,
  relay_delivery_json
)
select
  seq,
  record_id,
  family,
  status,
  source_runtime,
  created_at_ms,
  inserted_at_ms,
  updated_at_ms,
  owner_account_id,
  owner_pubkey,
  farm_id,
  listing_addr,
  local_work_json,
  event_id,
  event_kind,
  event_pubkey,
  event_created_at,
  event_tags_json,
  event_content,
  event_sig,
  raw_event_json,
  outbox_status,
  relay_set_fingerprint,
  relay_delivery_json
from local_event_record
order by seq asc;

drop table local_event_record;
alter table local_event_record_previous rename to local_event_record;

create index local_event_record_event_id_idx on local_event_record(event_id);
create index local_event_record_listing_addr_idx on local_event_record(listing_addr);
create index local_event_record_owner_pubkey_idx on local_event_record(owner_pubkey);
create index local_event_record_status_idx on local_event_record(status);
