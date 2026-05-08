//! Fork-only keybind bundle. Upstream's `Keybinds` carries a single
//! `fork: ForkKeybinds` field so upstream rebases conflict in one place.
//! Backing TOML stays flat on `KeysConfig` (keys.equalize_panes / picker).

use super::keybinds::ActionKeybinds;

#[derive(Debug, Clone, Default)]
pub struct ForkKeybinds {
    pub equalize_panes: ActionKeybinds,
    pub picker: ActionKeybinds,
}
