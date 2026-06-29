// Feature state lives in the registry, not on disk: the feature directory is
// read-only definition, but a feature's lifecycle position is mutable state, and
// all peios state is registry-native.
//
//   Machine\System\Features\<name>
//       State = REG_DWORD
//
// State is a single DWORD over a small state machine with transitional values,
// so an interrupted script leaves evidence (a feature stuck mid-`Installing`
// means install.sh never finished) rather than a quietly-wrong boolean:
//
//        0  NotInstalled
//        1  Installing     (0 -> 5, in progress)
//        5  Installed      (installed, not enabled)
//        6  Enabling       (5 -> 10, in progress)
//       10  Enabled
//        9  Disabling      (10 -> 5, in progress)
//        4  Uninstalling   (5 -> 0, in progress)
//
// Reads and writes happen under the caller's token, so KACS gates them: without
// authority to write the Machine hive, enabling a system feature returns EPERM.

use peios::registry::{CreateFlags, Key, KeyAccess, OpenFlags, ValueType};

use crate::error::{Error, Result};

const FEATURES_KEY: &str = r"Machine\System\Features";
const STATE: &[u8] = b"State";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    NotInstalled,
    Installing,
    Uninstalling,
    Installed,
    Enabling,
    Disabling,
    Enabled,
}

impl State {
    pub fn as_dword(self) -> u32 {
        match self {
            State::NotInstalled => 0,
            State::Installing => 1,
            State::Uninstalling => 4,
            State::Installed => 5,
            State::Enabling => 6,
            State::Disabling => 9,
            State::Enabled => 10,
        }
    }

    /// Decode the stored DWORD. Unknown values read as `NotInstalled` so a
    /// corrupt/foreign value fails safe to "do nothing is set up".
    pub fn from_dword(v: u32) -> State {
        match v {
            1 => State::Installing,
            4 => State::Uninstalling,
            5 => State::Installed,
            6 => State::Enabling,
            9 => State::Disabling,
            10 => State::Enabled,
            _ => State::NotInstalled,
        }
    }

    /// Human label for `feat list`.
    pub fn label(self) -> &'static str {
        match self {
            State::NotInstalled => "not-installed",
            State::Installing => "installing",
            State::Uninstalling => "uninstalling",
            State::Installed => "installed",
            State::Enabling => "enabling",
            State::Disabling => "disabling",
            State::Enabled => "enabled",
        }
    }
}

fn feature_key_path(name: &str) -> String {
    format!(r"{FEATURES_KEY}\{name}")
}

/// Read a feature's state. An absent key or value means `NotInstalled` (the
/// default), so a fresh system reads cleanly without provisioning.
pub fn read_state(name: &str) -> Result<State> {
    let path = feature_key_path(name);
    let key = match Key::open(None, &path, KeyAccess::READ, OpenFlags::default()) {
        Ok(key) => key,
        Err(e) if Error::is_enoent(&e) => return Ok(State::NotInstalled),
        Err(e) => return Err(Error::from_peios(format!("open {path}"), e)),
    };
    let dword = match key.query_value(STATE, None) {
        Ok(value) => value
            .data
            .get(0..4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(0),
        Err(_) => 0,
    };
    Ok(State::from_dword(dword))
}

/// Write a feature's state. Each transition writes the pending value *before*
/// running its script and the settled value *after* success, so a crash or
/// script failure leaves the pending value in the registry as evidence.
pub fn write_state(name: &str, state: State) -> Result<()> {
    let key = ensure_feature_key(name)?;
    let bytes = state.as_dword().to_le_bytes();
    key.set_value(STATE, ValueType::DWORD, &bytes)
        .call()
        .map_err(|e| Error::from_peios(format!("set State on {name}"), e))?;
    Ok(())
}

/// Open-or-create `Machine\System\Features\<name>`. `Key::create` does not
/// materialise intermediate keys, so the `Features` container is created first
/// (its parent `Machine\System` is provisioned by peinit at boot).
fn ensure_feature_key(name: &str) -> Result<Key> {
    let access = KeyAccess::WRITE | KeyAccess::SET_VALUE;
    Key::create(None, FEATURES_KEY, access, CreateFlags::empty(), None, None)
        .map_err(|e| Error::from_peios(format!("create {FEATURES_KEY}"), e))?;
    let path = feature_key_path(name);
    let (key, _disposition) = Key::create(None, &path, access, CreateFlags::empty(), None, None)
        .map_err(|e| Error::from_peios(format!("create {path}"), e))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::State;

    #[test]
    fn dword_values_match_the_spec() {
        assert_eq!(State::NotInstalled.as_dword(), 0);
        assert_eq!(State::Installing.as_dword(), 1);
        assert_eq!(State::Uninstalling.as_dword(), 4);
        assert_eq!(State::Installed.as_dword(), 5);
        assert_eq!(State::Enabling.as_dword(), 6);
        assert_eq!(State::Disabling.as_dword(), 9);
        assert_eq!(State::Enabled.as_dword(), 10);
    }

    #[test]
    fn dword_roundtrips() {
        for s in [
            State::NotInstalled,
            State::Installing,
            State::Uninstalling,
            State::Installed,
            State::Enabling,
            State::Disabling,
            State::Enabled,
        ] {
            assert_eq!(State::from_dword(s.as_dword()), s);
        }
    }

    #[test]
    fn unknown_dword_fails_safe_to_not_installed() {
        for v in [2, 3, 7, 8, 11, 99, 0xFFFF_FFFF] {
            assert_eq!(State::from_dword(v), State::NotInstalled);
        }
    }
}
