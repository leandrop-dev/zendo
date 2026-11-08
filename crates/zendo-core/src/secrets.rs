//! The API keys in the system keyring, through the Secret Service that
//! GNOME Keyring and KDE Wallet answer on D-Bus. Each key is an item with the
//! attributes `application = zendo` and `key = <name>`, so `secret-tool
//! lookup application zendo key nexus` finds it too.
//!
//! The keyring keeps them encrypted with the login password while it is
//! locked; once unlocked, any program of the user's may ask for them, as it
//! may read the settings file.

use std::collections::HashMap;

use anyhow::{Context, anyhow, bail};
use secret_service::blocking::SecretService;
use secret_service::{EncryptionType, Error};

/// An API key Zendo keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    SteamGridDb,
    Nexus,
    Steam,
}

impl Key {
    pub const ALL: [Self; 3] = [Self::SteamGridDb, Self::Nexus, Self::Steam];

    /// Its name in the keyring and in `config.toml`.
    fn name(self) -> &'static str {
        match self {
            Self::SteamGridDb => "steamgriddb",
            Self::Nexus => "nexus",
            Self::Steam => "steam",
        }
    }

    /// What a keyring program shows for it.
    fn label(self) -> &'static str {
        match self {
            Self::SteamGridDb => "Zendo: SteamGridDB API key",
            Self::Nexus => "Zendo: Nexus Mods API key",
            Self::Steam => "Zendo: Steam Web API key",
        }
    }

    fn attributes(self) -> HashMap<&'static str, &'static str> {
        HashMap::from([("application", "zendo"), ("key", self.name())])
    }
}

/// Where the keys are kept; the system keyring, or another in tests.
pub(crate) trait Keyring {
    /// The key, `None` when none is kept.
    fn get(&self, key: Key) -> anyhow::Result<Option<String>>;
    /// Keeps the key, or removes it with `None`.
    fn set(&self, key: Key, value: Option<&str>) -> anyhow::Result<()>;
}

/// The keyring the desktop provides.
pub(crate) struct System;

impl Keyring for System {
    /// Asks to unlock the keyring when it is locked: the desktop shows its
    /// password prompt, and this waits for it.
    fn get(&self, key: Key) -> anyhow::Result<Option<String>> {
        let service = connect()?;
        let found = service.search_items(key.attributes()).map_err(said)?;
        if found.unlocked.is_empty() && !found.locked.is_empty() {
            service
                .unlock_all(&found.locked.iter().collect::<Vec<_>>())
                .map_err(said)?;
        }
        let Some(item) = found.unlocked.first().or(found.locked.first()) else {
            return Ok(None);
        };
        let secret = item.get_secret().map_err(said)?;
        let text = String::from_utf8(secret).context("the key in the keyring is not text")?;
        Ok(Some(text))
    }

    /// Never asks to unlock: it runs on the window's thread, which a prompt
    /// would hold up. A locked keyring is an error, and the caller keeps
    /// the key elsewhere.
    fn set(&self, key: Key, value: Option<&str>) -> anyhow::Result<()> {
        let service = connect()?;
        if let Some(value) = value {
            let collection = service.get_default_collection().map_err(said)?;
            collection.ensure_unlocked().map_err(said)?;
            collection
                .create_item(
                    key.label(),
                    key.attributes(),
                    value.as_bytes(),
                    true,
                    "text/plain",
                )
                .map_err(said)?;
        } else {
            let found = service.search_items(key.attributes()).map_err(said)?;
            if !found.locked.is_empty() {
                bail!("the keyring is locked");
            }
            for item in found.unlocked {
                item.delete().map_err(said)?;
            }
        }
        Ok(())
    }
}

fn connect() -> anyhow::Result<SecretService<'static>> {
    SecretService::connect(EncryptionType::Dh).map_err(said)
}

/// The keyring's error, in words for the Settings page.
fn said(err: Error) -> anyhow::Error {
    match err {
        Error::Unavailable => anyhow!("no keyring answered (GNOME Keyring or KDE Wallet)"),
        Error::Locked => anyhow!("the keyring is locked"),
        Error::Prompt => anyhow!("the keyring was not unlocked"),
        Error::NoResult => anyhow!("the keyring has no default collection"),
        other => anyhow!("the keyring failed: {other}"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    /// A keyring in memory, which fails every call while `broken` is set.
    #[derive(Default)]
    pub(crate) struct Memory {
        pub keys: RefCell<HashMap<&'static str, String>>,
        pub broken: Cell<Option<&'static str>>,
    }

    impl Keyring for Memory {
        fn get(&self, key: Key) -> anyhow::Result<Option<String>> {
            if let Some(why) = self.broken.get() {
                bail!(why);
            }
            Ok(self.keys.borrow().get(key.name()).cloned())
        }

        fn set(&self, key: Key, value: Option<&str>) -> anyhow::Result<()> {
            if let Some(why) = self.broken.get() {
                bail!(why);
            }
            let mut keys = self.keys.borrow_mut();
            match value {
                Some(value) => keys.insert(key.name(), value.to_owned()),
                None => keys.remove(key.name()),
            };
            Ok(())
        }
    }

    /// Writes, reads and removes a key in the session's real keyring, and
    /// is refused, not prompted, once it is locked. Run it in a throwaway
    /// keyring with folders of its own, so the user's keyring and its
    /// daemon are never touched (`<test binary>` from `cargo test --no-run`):
    ///
    /// ```sh
    /// mkdir -p /tmp/k/home /tmp/k/data /tmp/k/run && chmod 700 /tmp/k/run
    /// env -i PATH=/usr/bin HOME=/tmp/k/home XDG_DATA_HOME=/tmp/k/data \
    ///   XDG_RUNTIME_DIR=/tmp/k/run ZENDO_TEST_KEYRING=1 dbus-run-session -- \
    ///   sh -c 'echo pw | gnome-keyring-daemon --unlock --components=secrets \
    ///   >/dev/null; <test binary> keyring --ignored'
    /// ```
    #[test]
    #[ignore = "needs a throwaway Secret Service; see the comment"]
    fn the_system_keyring_keeps_and_forgets_a_key() {
        assert!(
            std::env::var_os("ZENDO_TEST_KEYRING").is_some(),
            "set ZENDO_TEST_KEYRING inside a throwaway keyring session"
        );
        System.set(Key::Nexus, Some("test-key")).unwrap();
        assert_eq!(System.get(Key::Nexus).unwrap().as_deref(), Some("test-key"));
        System.set(Key::Nexus, Some("replaced")).unwrap();
        assert_eq!(System.get(Key::Nexus).unwrap().as_deref(), Some("replaced"));
        System.set(Key::Nexus, None).unwrap();
        assert_eq!(System.get(Key::Nexus).unwrap(), None);
        assert_eq!(System.get(Key::SteamGridDb).unwrap(), None);
        let service = connect().unwrap();
        service.get_default_collection().unwrap().lock().unwrap();
        let refused = System.set(Key::Nexus, Some("locked")).unwrap_err();
        assert_eq!(refused.to_string(), "the keyring is locked");
    }
}
