use crate::error::RadrootsNostrAccountsError;
use crate::manager::RadrootsNostrAccountsManager;
use radroots_nostr_ndb::prelude::RadrootsNostrNdb;

pub fn radroots_nostr_accounts_register_default_secret_with_ndb(
    manager: &RadrootsNostrAccountsManager,
    ndb: &RadrootsNostrNdb,
) -> Result<bool, RadrootsNostrAccountsError> {
    let Some(signer) = manager.default_signer_capability()? else {
        return Ok(false);
    };
    let Some(identity) = manager.resolve_signing_identity_for_signer(&signer)? else {
        return Ok(false);
    };
    Ok(ndb.add_giftwrap_secret_key(identity.secret_key_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::RadrootsNostrFileAccountStore;
    use crate::vault::RadrootsNostrSecretVaultMemory;
    use radroots_nostr_ndb::prelude::RadrootsNostrNdbConfig;
    use std::sync::Arc;

    #[test]
    fn register_default_secret_returns_true_for_signing_account() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Arc::new(RadrootsNostrFileAccountStore::new(
            temp.path().join("accounts.json"),
        ));
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let manager = RadrootsNostrAccountsManager::new(store, vault).expect("manager");
        manager
            .generate_identity(Some("primary".into()), true)
            .expect("generate");

        let ndb = RadrootsNostrNdb::open(RadrootsNostrNdbConfig::new(temp.path().join("ndb")))
            .expect("ndb");
        let added = radroots_nostr_accounts_register_default_secret_with_ndb(&manager, &ndb)
            .expect("register");
        assert!(added);
    }

    #[test]
    fn register_default_secret_returns_false_for_watch_only_account() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = Arc::new(RadrootsNostrFileAccountStore::new(
            temp.path().join("accounts.json"),
        ));
        let vault = Arc::new(RadrootsNostrSecretVaultMemory::new());
        let manager = RadrootsNostrAccountsManager::new(store, vault).expect("manager");
        manager
            .upsert_public_identity(
                radroots_identity::RadrootsIdentity::generate().to_public(),
                Some("watch".into()),
                true,
            )
            .expect("watch");

        let ndb = RadrootsNostrNdb::open(RadrootsNostrNdbConfig::new(temp.path().join("ndb")))
            .expect("ndb");
        let added = radroots_nostr_accounts_register_default_secret_with_ndb(&manager, &ndb)
            .expect("register");
        assert!(!added);
    }
}
