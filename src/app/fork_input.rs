//! Fork-owned prefix-mode key dispatch.
//!
//! Upstream's `navigate.rs::handle_prefix_key` calls `handle_fork_prefix_key`
//! at a single hook so adding a fork binding never re-touches the upstream
//! prefix handler. equalize_panes is intentionally NOT here: it routes through
//! the upstream `NavigateAction` action table. Only bindings that bypass that
//! table live here.

use crate::app::App;
use crate::input::TerminalKey;

impl App {
    /// Dispatch fork-owned prefix-mode bindings. Returns true if consumed.
    pub(crate) fn handle_fork_prefix_key(&mut self, raw_key: TerminalKey) -> bool {
        if self.state.keybinds.fork.picker.matches_prefix_key(raw_key) {
            self.toggle_picker();
            return true;
        }
        false
    }
}
