//! Surface picker — fuzzy finder across workspaces, tabs, panes, and agents.
//!
//! Snapshots the navigable surfaces on open into a flat list of entries,
//! ranks them against the user's query with `nucleo_matcher`, and on Enter
//! activates the chosen entry by switching workspace + tab and focusing the
//! pane as needed. Visibility/active-ness is a single `Option<PickerState>`
//! field on `AppState`, input is intercepted early in the input pipeline,
//! render is appended after the mode match.

use crossterm::event::{KeyCode, KeyModifiers};
use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Matcher, Utf32Str,
};

use crate::detect::agent_label;
use crate::input::TerminalKey;
use crate::layout::PaneId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Workspace,
    Tab,
    Pane,
    Agent,
}

#[derive(Debug, Clone, Copy)]
pub struct PickerTarget {
    pub workspace_idx: usize,
    pub tab_idx: Option<usize>,
    pub pane_id: Option<PaneId>,
}

pub struct PickerEntry {
    pub kind: PickerKind,
    pub target: PickerTarget,
    pub label: String,
    pub secondary: String,
}

pub struct PickerMatch {
    pub entry_idx: usize,
    pub indices: Vec<u32>,
}

pub struct PickerState {
    pub query: String,
    pub filter: Option<PickerKind>,
    pub entries: Vec<PickerEntry>,
    pub matches: Vec<PickerMatch>,
    pub selected: usize,
    pub matcher: Matcher,
}

const FILTER_CYCLE: [Option<PickerKind>; 5] = [
    None,
    Some(PickerKind::Workspace),
    Some(PickerKind::Tab),
    Some(PickerKind::Pane),
    Some(PickerKind::Agent),
];

fn cycle_filter(current: Option<PickerKind>, forward: bool) -> Option<PickerKind> {
    let pos = FILTER_CYCLE.iter().position(|f| *f == current).unwrap_or(0);
    let len = FILTER_CYCLE.len();
    let next = if forward {
        (pos + 1) % len
    } else {
        (pos + len - 1) % len
    };
    FILTER_CYCLE[next]
}

fn sigil_to_kind(c: char) -> Option<PickerKind> {
    match c {
        '>' => Some(PickerKind::Workspace),
        '#' => Some(PickerKind::Tab),
        ':' => Some(PickerKind::Pane),
        '@' => Some(PickerKind::Agent),
        _ => None,
    }
}

/// True while keys should route to the picker instead of the active surface.
pub(crate) fn is_active(state: &super::AppState) -> bool {
    state.picker.is_some()
}

impl super::App {
    /// Toggle picker open/closed. Snapshots entries on open. Closes if open.
    pub(crate) fn toggle_picker(&mut self) {
        if self.state.picker.is_some() {
            self.state.picker = None;
            return;
        }
        if self.state.workspaces.is_empty() {
            return;
        }
        let entries = build_entries(&self.state, &self.terminal_runtimes);
        if entries.is_empty() {
            return;
        }
        let mut picker = PickerState {
            query: String::new(),
            filter: None,
            entries,
            matches: Vec::new(),
            selected: 0,
            matcher: Matcher::new(nucleo_matcher::Config::DEFAULT),
        };
        refresh_matches(&mut picker);
        self.state.picker = Some(picker);
    }

    /// Forward a key to the picker. Caller must have verified `is_active()`.
    pub(crate) fn forward_key_to_picker(&mut self, key: TerminalKey) {
        let key_event = key.as_key_event();
        if self.state.is_prefix_key(key) {
            self.state.picker = None;
            self.state.mode = super::Mode::Navigate;
            return;
        }

        match (key_event.code, key_event.modifiers) {
            (KeyCode::Esc, _) => {
                self.state.picker = None;
            }
            (KeyCode::Enter, _) => {
                activate(self);
            }
            (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                if let Some(picker) = self.state.picker.as_mut() {
                    if picker.selected > 0 {
                        picker.selected -= 1;
                    }
                }
            }
            (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                if let Some(picker) = self.state.picker.as_mut() {
                    if picker.selected + 1 < picker.matches.len() {
                        picker.selected += 1;
                    }
                }
            }
            (KeyCode::Tab, _) => {
                if let Some(picker) = self.state.picker.as_mut() {
                    picker.filter = cycle_filter(picker.filter, true);
                    refresh_matches(picker);
                }
            }
            (KeyCode::BackTab, _) => {
                if let Some(picker) = self.state.picker.as_mut() {
                    picker.filter = cycle_filter(picker.filter, false);
                    refresh_matches(picker);
                }
            }
            (KeyCode::Backspace, _) => {
                if let Some(picker) = self.state.picker.as_mut() {
                    if picker.query.is_empty() && picker.filter.is_some() {
                        picker.filter = None;
                    } else {
                        picker.query.pop();
                    }
                    refresh_matches(picker);
                }
            }
            (KeyCode::Char(c), mods)
                if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::ALT) =>
            {
                if let Some(picker) = self.state.picker.as_mut() {
                    if picker.query.is_empty() && picker.filter.is_none() {
                        if let Some(kind) = sigil_to_kind(c) {
                            picker.filter = Some(kind);
                            refresh_matches(picker);
                            return;
                        }
                    }
                    picker.query.push(c);
                    refresh_matches(picker);
                }
            }
            _ => {}
        }
    }
}

fn build_entries(
    state: &super::AppState,
    terminal_runtimes: &crate::terminal::TerminalRuntimeRegistry,
) -> Vec<PickerEntry> {
    let mut entries = Vec::new();
    for (ws_idx, ws) in state.workspaces.iter().enumerate() {
        let ws_name = ws.display_name();
        entries.push(PickerEntry {
            kind: PickerKind::Workspace,
            target: PickerTarget {
                workspace_idx: ws_idx,
                tab_idx: None,
                pane_id: None,
            },
            label: ws_name.clone(),
            secondary: format!("{} tabs", ws.tabs.len()),
        });

        for (tab_idx, tab) in ws.tabs.iter().enumerate() {
            let scope = if tab.is_auto_named() {
                ws_name.clone()
            } else {
                format!("{ws_name} › {}", tab.display_name())
            };
            entries.push(PickerEntry {
                kind: PickerKind::Tab,
                target: PickerTarget {
                    workspace_idx: ws_idx,
                    tab_idx: Some(tab_idx),
                    pane_id: None,
                },
                label: scope.clone(),
                secondary: format!("{} panes", tab.panes.len()),
            });

            for (pane_id, pane_state) in &tab.panes {
                let cwd = tab
                    .cwd_for_pane(*pane_id, &state.terminals, terminal_runtimes)
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                entries.push(PickerEntry {
                    kind: PickerKind::Pane,
                    target: PickerTarget {
                        workspace_idx: ws_idx,
                        tab_idx: Some(tab_idx),
                        pane_id: Some(*pane_id),
                    },
                    label: scope.clone(),
                    secondary: cwd.clone(),
                });

                let terminal = state.terminals.get(&pane_state.attached_terminal_id);
                if let Some(label_str) = terminal.and_then(|t| {
                    t.agent_name.clone().or_else(|| {
                        t.effective_known_agent()
                            .map(|a| agent_label(a).to_string())
                    })
                }) {
                    let agent_state = terminal
                        .map(|t| t.state)
                        .unwrap_or(crate::detect::AgentState::Unknown);
                    let state_label = format!("{agent_state:?}").to_lowercase();
                    let agent_secondary = if cwd.is_empty() {
                        state_label
                    } else {
                        format!("{state_label} · {cwd}")
                    };
                    entries.push(PickerEntry {
                        kind: PickerKind::Agent,
                        target: PickerTarget {
                            workspace_idx: ws_idx,
                            tab_idx: Some(tab_idx),
                            pane_id: Some(*pane_id),
                        },
                        label: format!("{label_str} · {scope}"),
                        secondary: agent_secondary,
                    });
                }
            }
        }
    }
    entries
}

fn refresh_matches(picker: &mut PickerState) {
    picker.matches.clear();
    let filter = picker.filter;
    let passes_filter =
        |entry: &PickerEntry| -> bool { filter.is_none_or(|kind| entry.kind == kind) };

    if picker.query.is_empty() {
        for (idx, entry) in picker.entries.iter().enumerate() {
            if passes_filter(entry) {
                picker.matches.push(PickerMatch {
                    entry_idx: idx,
                    indices: Vec::new(),
                });
            }
        }
    } else {
        let pattern = Pattern::parse(&picker.query, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(u32, PickerMatch)> = Vec::new();
        for (idx, entry) in picker.entries.iter().enumerate() {
            if !passes_filter(entry) {
                continue;
            }
            buf.clear();
            let haystack = Utf32Str::new(&entry.label, &mut buf);
            let mut indices: Vec<u32> = Vec::new();
            if let Some(score) = pattern.indices(haystack, &mut picker.matcher, &mut indices) {
                indices.sort_unstable();
                indices.dedup();
                scored.push((
                    score,
                    PickerMatch {
                        entry_idx: idx,
                        indices,
                    },
                ));
            }
        }
        scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        picker.matches.extend(scored.into_iter().map(|(_, m)| m));
    }

    if picker.matches.is_empty() {
        picker.selected = 0;
    } else if picker.selected >= picker.matches.len() {
        picker.selected = picker.matches.len() - 1;
    }
}

fn activate(app: &mut super::App) {
    let Some(picker) = app.state.picker.as_ref() else {
        return;
    };
    let Some(m) = picker.matches.get(picker.selected) else {
        app.state.picker = None;
        return;
    };
    let Some(entry) = picker.entries.get(m.entry_idx) else {
        app.state.picker = None;
        return;
    };
    let target = entry.target;
    app.state.picker = None;

    if target.workspace_idx >= app.state.workspaces.len() {
        return;
    }
    app.state.switch_workspace(target.workspace_idx);

    if let Some(tab_idx) = target.tab_idx {
        let tab_count = app
            .state
            .workspaces
            .get(target.workspace_idx)
            .map(|ws| ws.tabs.len())
            .unwrap_or(0);
        if tab_idx >= tab_count {
            return;
        }
        app.state.switch_tab(tab_idx);

        if let Some(pane_id) = target.pane_id {
            if let Some(ws) = app.state.workspaces.get_mut(target.workspace_idx) {
                if let Some(tab) = ws.tabs.get_mut(tab_idx) {
                    if tab.panes.contains_key(&pane_id) {
                        tab.layout.focus_pane(pane_id);
                    }
                }
            }
        }
    }

    if app.state.active.is_some() {
        app.state.mode = super::Mode::Terminal;
    }
}
