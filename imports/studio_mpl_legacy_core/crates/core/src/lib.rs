//! UI-neutral Radroots Studio application core.
//!
//! This crate owns the canonical process-global application state. Native UI
//! shells receive immutable snapshots through UniFFI and submit explicit state
//! transitions back to Rust. No Compose, AppKit, Swing, browser, or other UI
//! types cross this boundary.

use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

const SCHEMA_VERSION: u32 = 1;
const SAMPLE_ACCOUNT_ID: &str = "account-tyson-lupul";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AccountLoginStatus {
    LoggedIn,
    LoggedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum UpdateChannel {
    Stable,
    Preview,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct AccountState {
    pub id: String,
    pub display_name: String,
    pub network_name: String,
    pub email: String,
    pub login_status: AccountLoginStatus,
    pub expiry_summary: String,
    pub avatar_asset_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct PreferencesState {
    pub allow_incoming_connections: bool,
    pub use_radroots_dns: bool,
    pub use_radroots_subnets: bool,
    pub launch_at_login: bool,
    pub hide_dock_icon: bool,
    pub vpn_on_demand_enabled: bool,
    pub run_as_exit_node: bool,
    pub allow_local_network_access: bool,
    pub automatically_check_for_updates: bool,
    pub update_channel: UpdateChannel,
    pub last_update_check_summary: String,
    pub alternate_server_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct GlobalAppState {
    pub schema_version: u32,
    pub revision: u64,
    pub accounts: Vec<AccountState>,
    pub selected_account_id: Option<String>,
    pub preferences: PreferencesState,
}

static GLOBAL_APP_STATE: OnceLock<RwLock<GlobalAppState>> = OnceLock::new();

fn initial_state() -> GlobalAppState {
    GlobalAppState {
        schema_version: SCHEMA_VERSION,
        revision: 1,
        accounts: vec![AccountState {
            id: SAMPLE_ACCOUNT_ID.to_owned(),
            display_name: "Tyson Lupul".to_owned(),
            network_name: "triesap.github".to_owned(),
            email: "triesap@github".to_owned(),
            login_status: AccountLoginStatus::LoggedOut,
            expiry_summary: "Expires in 4 months".to_owned(),
            avatar_asset_key: "radroots_sample_account_avatar".to_owned(),
        }],
        selected_account_id: Some(SAMPLE_ACCOUNT_ID.to_owned()),
        preferences: PreferencesState {
            allow_incoming_connections: true,
            use_radroots_dns: true,
            use_radroots_subnets: true,
            launch_at_login: true,
            hide_dock_icon: false,
            vpn_on_demand_enabled: false,
            run_as_exit_node: false,
            allow_local_network_access: false,
            automatically_check_for_updates: true,
            update_channel: UpdateChannel::Stable,
            last_update_check_summary: "Today, 20:31".to_owned(),
            alternate_server_url: String::new(),
        },
    }
}

fn state_lock() -> &'static RwLock<GlobalAppState> {
    GLOBAL_APP_STATE.get_or_init(|| RwLock::new(initial_state()))
}

fn read_state() -> RwLockReadGuard<'static, GlobalAppState> {
    state_lock()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_state() -> RwLockWriteGuard<'static, GlobalAppState> {
    state_lock()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Applies a transition to a candidate snapshot, then commits it atomically.
///
/// This prevents partially-applied state if a future transition grows to touch
/// multiple fields. Revisions advance only when the canonical value changes.
fn commit_if_changed(
    state: &mut GlobalAppState,
    mutate: impl FnOnce(&mut GlobalAppState),
) -> GlobalAppState {
    let before = state.clone();
    let mut candidate = before.clone();
    mutate(&mut candidate);

    if candidate != before {
        candidate.revision = before.revision.saturating_add(1);
        *state = candidate;
    }

    state.clone()
}

#[uniffi::export]
pub fn initialize_global_app_state() -> GlobalAppState {
    state_lock();
    read_state().clone()
}

#[uniffi::export]
pub fn read_global_app_state() -> GlobalAppState {
    read_state().clone()
}

#[uniffi::export]
pub fn export_global_app_state_json() -> String {
    serde_json::to_string_pretty(&*read_state()).unwrap_or_else(|error| {
        serde_json::json!({
            "error": format!("failed to serialize global app state: {error}"),
        })
        .to_string()
    })
}

#[uniffi::export]
pub fn select_account(account_id: String) -> GlobalAppState {
    let mut state = write_state();
    commit_if_changed(&mut state, |candidate| {
        if candidate
            .accounts
            .iter()
            .any(|account| account.id == account_id)
        {
            candidate.selected_account_id = Some(account_id);
        }
    })
}

#[uniffi::export]
pub fn set_selected_account_login_status(status: AccountLoginStatus) -> GlobalAppState {
    let mut state = write_state();
    commit_if_changed(&mut state, |candidate| {
        let Some(selected_id) = candidate.selected_account_id.clone() else {
            return;
        };
        if let Some(account) = candidate
            .accounts
            .iter_mut()
            .find(|account| account.id == selected_id)
        {
            account.login_status = status;
        }
    })
}

#[uniffi::export]
pub fn remove_selected_account() -> GlobalAppState {
    let mut state = write_state();
    commit_if_changed(&mut state, |candidate| {
        let Some(selected_id) = candidate.selected_account_id.take() else {
            return;
        };
        candidate
            .accounts
            .retain(|account| account.id != selected_id);
        candidate.selected_account_id =
            candidate.accounts.first().map(|account| account.id.clone());
    })
}

#[uniffi::export]
pub fn set_allow_incoming_connections(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.allow_incoming_connections = value)
}

#[uniffi::export]
pub fn set_use_radroots_dns(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.use_radroots_dns = value)
}

#[uniffi::export]
pub fn set_use_radroots_subnets(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.use_radroots_subnets = value)
}

#[uniffi::export]
pub fn set_launch_at_login(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.launch_at_login = value)
}

#[uniffi::export]
pub fn set_hide_dock_icon(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.hide_dock_icon = value)
}

#[uniffi::export]
pub fn set_vpn_on_demand_enabled(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.vpn_on_demand_enabled = value)
}

#[uniffi::export]
pub fn set_run_as_exit_node(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.run_as_exit_node = value)
}

#[uniffi::export]
pub fn set_allow_local_network_access(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.allow_local_network_access = value)
}

#[uniffi::export]
pub fn set_automatically_check_for_updates(value: bool) -> GlobalAppState {
    update_preferences(|preferences| preferences.automatically_check_for_updates = value)
}

#[uniffi::export]
pub fn set_update_channel(value: UpdateChannel) -> GlobalAppState {
    update_preferences(|preferences| preferences.update_channel = value)
}

#[uniffi::export]
pub fn set_alternate_server_url(value: String) -> GlobalAppState {
    update_preferences(|preferences| preferences.alternate_server_url = value.trim().to_owned())
}

fn update_preferences(mutate: impl FnOnce(&mut PreferencesState)) -> GlobalAppState {
    let mut state = write_state();
    commit_if_changed(&mut state, |candidate| {
        mutate(&mut candidate.preferences);
    })
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_contains_the_reviewed_account_fixture() {
        let state = initial_state();
        let account = state.accounts.first().expect("sample account");
        assert_eq!(account.display_name, "Tyson Lupul");
        assert_eq!(account.network_name, "triesap.github");
        assert_eq!(account.email, "triesap@github");
        assert_eq!(account.login_status, AccountLoginStatus::LoggedOut);
        assert_eq!(
            state.selected_account_id.as_deref(),
            Some(SAMPLE_ACCOUNT_ID)
        );
    }

    #[test]
    fn candidate_commit_increments_revision_only_when_state_changes() {
        let mut state = initial_state();
        let first_revision = state.revision;
        let unchanged = commit_if_changed(&mut state, |_| {});
        assert_eq!(unchanged.revision, first_revision);

        let changed = commit_if_changed(&mut state, |candidate| {
            candidate.preferences.hide_dock_icon = true;
        });
        assert_eq!(changed.revision, first_revision + 1);
    }
}
