//! Fork overlays: rendered above panes, below notifications.
//!
//! Upstream's render path calls `fork::render_overlays` at a single hook so
//! adding a fork overlay never re-touches the upstream render path.

use ratatui::{layout::Rect, Frame};

use crate::app::AppState;

pub(super) fn render_overlays(app: &AppState, frame: &mut Frame, terminal_area: Rect) {
    super::picker::render(app, frame, terminal_area);
}
