use keyring::Entry;

pub const KEYCHAIN_SERVICE: &str = "language-translation-app";

/// Secure credential storage backed by the OS Keychain / Credential Manager
/// (macOS Keychain, Windows Credential Manager, Linux Secret Service).
///
/// Operations fail gracefully (returning `None` / `false`) in headless or
/// restricted environments so the app can transparently fall back to local JSON.
pub struct SecretStore;

impl SecretStore {
    pub fn account_name(provider_id: &str) -> String {
        format!("api_key_{}", provider_id.trim().to_ascii_lowercase())
    }

    pub fn set_secret(provider_id: &str, secret: &str) -> bool {
        let account = Self::account_name(provider_id);
        let Ok(entry) = Entry::new(KEYCHAIN_SERVICE, &account) else {
            return false;
        };
        if secret.trim().is_empty() {
            let _ = entry.delete_credential();
            return true;
        }
        entry.set_password(secret.trim()).is_ok()
    }

    pub fn get_secret(provider_id: &str) -> Option<String> {
        let account = Self::account_name(provider_id);
        let entry = Entry::new(KEYCHAIN_SERVICE, &account).ok()?;
        let val = entry.get_password().ok()?;
        if val.trim().is_empty() {
            None
        } else {
            Some(val)
        }
    }

    pub fn delete_secret(provider_id: &str) -> bool {
        let account = Self::account_name(provider_id);
        let Ok(entry) = Entry::new(KEYCHAIN_SERVICE, &account) else {
            return false;
        };
        entry.delete_credential().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_consistent_account_names() {
        assert_eq!(SecretStore::account_name("OpenAI"), "api_key_openai");
        assert_eq!(SecretStore::account_name("  Gemini "), "api_key_gemini");
        assert_eq!(SecretStore::account_name("Claude"), "api_key_claude");
    }
}
