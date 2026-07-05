//! OS-keychain-backed storage for the refresh token (macOS Keychain / Windows Credential
//! Manager / Linux Secret Service via the `keyring` crate). The access token is never
//! persisted here — it lives only in `SigninState`'s in-memory `Mutex`.

use keyring::Entry;
use std::sync::Once;

const SERVICE: &str = "com.skillshome.desktop";
const REFRESH_TOKEN_KEY: &str = "refresh_token";

static INIT_STORE: Once = Once::new();

/// `keyring` v4's own lazy default-store init has a bug (its one-time-init guard in
/// `keyring::v1::Entry::new` can never fire — verified in keyring-4.1.3/src/v1.rs), so
/// `keyring_core::set_default_store` never gets called and every `Entry::new` fails with
/// `NoDefaultStore`. We do the same init ourselves, once, before touching any entry.
fn ensure_default_store() {
    INIT_STORE.call_once(|| {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let store = apple_native_keyring_store::keychain::Store::new();
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new();
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
        let store = zbus_secret_service_keyring_store::Store::new();

        match store {
            Ok(store) => keyring_core::set_default_store(store),
            Err(e) => eprintln!("failed to initialize OS credential store: {e}"),
        }
    });
}

fn entry() -> Result<Entry, String> {
    ensure_default_store();
    Entry::new(SERVICE, REFRESH_TOKEN_KEY).map_err(|e| e.to_string())
}

pub fn save_refresh_token(token: &str) -> Result<(), String> {
    entry()?.set_password(token).map_err(|e| e.to_string())
}

pub fn load_refresh_token() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn delete_refresh_token() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Every test in this crate that touches the real OS keychain uses this same fixed
/// `(SERVICE, REFRESH_TOKEN_KEY)` entry (there's only one refresh token per install, so
/// there's nothing to parameterize a key by). Cargo runs tests in one binary in parallel
/// by default, so any such test — in this file or `silent_refresh.rs` — must hold this
/// lock for its duration to avoid racing another test's save/load/delete.
#[cfg(test)]
pub(crate) static KEYCHAIN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_delete_round_trip() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());

        delete_refresh_token().expect("pre-test cleanup should not fail");

        assert_eq!(load_refresh_token().unwrap(), None);

        save_refresh_token("test-refresh-token-value").unwrap();
        assert_eq!(
            load_refresh_token().unwrap(),
            Some("test-refresh-token-value".to_string())
        );

        delete_refresh_token().unwrap();
        assert_eq!(load_refresh_token().unwrap(), None);
    }

    #[test]
    fn delete_when_absent_is_not_an_error() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());

        delete_refresh_token().unwrap();
        assert!(delete_refresh_token().is_ok());
    }
}
