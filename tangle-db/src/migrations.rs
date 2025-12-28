use radroots_sql_core::SqlExecutor;
use radroots_sql_core::error::SqlError;
use radroots_sql_core::migrations::{Migration, migrations_run_all_down, migrations_run_all_up};

pub static MIGRATIONS: &[Migration] = &[
    Migration {
        name: "0000_init",
        up_sql: include_str!("../migrations/0000_init.up.sql"),
        down_sql: include_str!("../migrations/0000_init.down.sql"),
    },
    Migration {
        name: "0001_log_error",
        up_sql: include_str!("../migrations/0001_log_error.up.sql"),
        down_sql: include_str!("../migrations/0001_log_error.down.sql"),
    },
    Migration {
        name: "0002_farm",
        up_sql: include_str!("../migrations/0002_farm.up.sql"),
        down_sql: include_str!("../migrations/0002_farm.down.sql"),
    },
    Migration {
        name: "0003_gcs_location",
        up_sql: include_str!("../migrations/0003_gcs_location.up.sql"),
        down_sql: include_str!("../migrations/0003_gcs_location.down.sql"),
    },
    Migration {
        name: "0004_trade_product",
        up_sql: include_str!("../migrations/0004_trade_product.up.sql"),
        down_sql: include_str!("../migrations/0004_trade_product.down.sql"),
    },
    Migration {
        name: "0005_nostr_profile",
        up_sql: include_str!("../migrations/0005_nostr_profile.up.sql"),
        down_sql: include_str!("../migrations/0005_nostr_profile.down.sql"),
    },
    Migration {
        name: "0006_nostr_relay",
        up_sql: include_str!("../migrations/0006_nostr_relay.up.sql"),
        down_sql: include_str!("../migrations/0006_nostr_relay.down.sql"),
    },
    Migration {
        name: "0007_media_image",
        up_sql: include_str!("../migrations/0007_media_image.up.sql"),
        down_sql: include_str!("../migrations/0007_media_image.down.sql"),
    },
    Migration {
        name: "0008_farm_gcs_location",
        up_sql: include_str!("../migrations/0008_farm_gcs_location.up.sql"),
        down_sql: include_str!("../migrations/0008_farm_gcs_location.down.sql"),
    },
    Migration {
        name: "0009_nostr_profile_relay",
        up_sql: include_str!("../migrations/0009_nostr_profile_relay.up.sql"),
        down_sql: include_str!("../migrations/0009_nostr_profile_relay.down.sql"),
    },
    Migration {
        name: "0010_trade_product_location",
        up_sql: include_str!("../migrations/0010_trade_product_location.up.sql"),
        down_sql: include_str!("../migrations/0010_trade_product_location.down.sql"),
    },
    Migration {
        name: "0011_trade_product_media",
        up_sql: include_str!("../migrations/0011_trade_product_media.up.sql"),
        down_sql: include_str!("../migrations/0011_trade_product_media.down.sql"),
    },
    Migration {
        name: "0012_plot",
        up_sql: include_str!("../migrations/0012_plot.up.sql"),
        down_sql: include_str!("../migrations/0012_plot.down.sql"),
    },
    Migration {
        name: "0013_plot_gcs_location",
        up_sql: include_str!("../migrations/0013_plot_gcs_location.up.sql"),
        down_sql: include_str!("../migrations/0013_plot_gcs_location.down.sql"),
    },
    Migration {
        name: "0014_farm_tag",
        up_sql: include_str!("../migrations/0014_farm_tag.up.sql"),
        down_sql: include_str!("../migrations/0014_farm_tag.down.sql"),
    },
    Migration {
        name: "0015_plot_tag",
        up_sql: include_str!("../migrations/0015_plot_tag.up.sql"),
        down_sql: include_str!("../migrations/0015_plot_tag.down.sql"),
    },
    Migration {
        name: "0016_farm_member",
        up_sql: include_str!("../migrations/0016_farm_member.up.sql"),
        down_sql: include_str!("../migrations/0016_farm_member.down.sql"),
    },
    Migration {
        name: "0017_farm_member_claim",
        up_sql: include_str!("../migrations/0017_farm_member_claim.up.sql"),
        down_sql: include_str!("../migrations/0017_farm_member_claim.down.sql"),
    },
    Migration {
        name: "0018_nostr_event_state",
        up_sql: include_str!("../migrations/0018_nostr_event_state.up.sql"),
        down_sql: include_str!("../migrations/0018_nostr_event_state.down.sql"),
    },
];

pub fn run_all_up<E>(executor: &E) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    migrations_run_all_up(executor, MIGRATIONS)
}

pub fn run_all_down<E>(executor: &E) -> Result<(), SqlError>
where
    E: SqlExecutor,
{
    migrations_run_all_down(executor, MIGRATIONS)
}
