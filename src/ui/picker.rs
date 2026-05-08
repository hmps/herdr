//! Render layer for the surface picker (fuzzy finder over workspaces, tabs,
//! panes, and detected agents).

use ratatui::{
    buffer::Cell,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::widgets::render_modal_shell;
use crate::app::picker::{PickerKind, PickerState};
use crate::app::AppState;

pub(super) fn render(app: &AppState, frame: &mut Frame, area: Rect) {
    let Some(picker) = app.picker.as_ref() else {
        return;
    };
    let Some(popup) = popup_rect(area) else {
        return;
    };

    paint_backdrop(frame, area, &app.palette);
    let Some(inner) = render_modal_shell(frame, area, popup.width, popup.height, &app.palette)
    else {
        return;
    };

    let [query_row, _gap, list_area, footer_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas::<4>(inner);

    let chip_text = match picker.filter {
        Some(kind) => kind_tag(kind),
        None => "all",
    };
    let mut query_spans = vec![
        Span::styled(" › ", Style::default().fg(app.palette.accent)),
        Span::styled(
            format!("[{chip_text}]"),
            Style::default()
                .bg(app.palette.surface1)
                .fg(app.palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];
    query_spans.push(Span::styled(
        &picker.query,
        Style::default()
            .fg(app.palette.text)
            .add_modifier(Modifier::BOLD),
    ));
    query_spans.push(Span::styled("_", Style::default().fg(app.palette.overlay1)));
    frame.render_widget(Paragraph::new(Line::from(query_spans)), query_row);

    render_match_list(frame, list_area, picker, &app.palette);

    let footer = Line::from(vec![Span::styled(
        " enter select · esc close · ↑↓ move · tab filter",
        Style::default().fg(app.palette.overlay0),
    )]);
    frame.render_widget(Paragraph::new(footer), footer_row);
}

fn render_match_list(
    frame: &mut Frame,
    area: Rect,
    picker: &PickerState,
    palette: &crate::app::state::Palette,
) {
    if area.height == 0 {
        return;
    }
    if picker.matches.is_empty() {
        let line = Line::from(vec![Span::styled(
            "  no matches",
            Style::default().fg(palette.overlay0),
        )]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let visible = area.height as usize;
    let start = picker
        .selected
        .saturating_sub(visible.saturating_sub(1).min(picker.selected));
    let end = (start + visible).min(picker.matches.len());

    let name_col_width = picker
        .matches
        .iter()
        .filter_map(|m| picker.entries.get(m.entry_idx))
        .map(|e| e.label.chars().count())
        .max()
        .unwrap_or(0)
        .min(40);

    for (row_idx, match_idx) in (start..end).enumerate() {
        let m = &picker.matches[match_idx];
        let Some(entry) = picker.entries.get(m.entry_idx) else {
            continue;
        };
        let row_rect = Rect::new(area.x, area.y + row_idx as u16, area.width, 1);
        let is_selected = match_idx == picker.selected;

        let base_style = if is_selected {
            Style::default()
                .bg(palette.surface0)
                .fg(palette.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text)
        };
        let highlight_style = base_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        let dim_style = if is_selected {
            Style::default().bg(palette.surface0).fg(palette.overlay1)
        } else {
            Style::default().fg(palette.overlay0)
        };
        let tag_style = if is_selected {
            Style::default().bg(palette.surface0).fg(palette.accent)
        } else {
            Style::default().fg(palette.mauve)
        };

        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!(" {:7} ", kind_tag(entry.kind)),
            tag_style,
        ));

        let label_chars: Vec<char> = entry.label.chars().collect();
        let mut byte_to_char: Vec<usize> = Vec::with_capacity(entry.label.len() + 1);
        let mut char_idx = 0;
        for (b, _) in entry.label.char_indices() {
            while byte_to_char.len() <= b {
                byte_to_char.push(char_idx);
            }
            char_idx += 1;
        }
        while byte_to_char.len() <= entry.label.len() {
            byte_to_char.push(char_idx);
        }

        let label_char_count = label_chars.len();
        for (i, ch) in label_chars.iter().enumerate() {
            let highlight = m
                .indices
                .iter()
                .any(|&idx| byte_to_char.get(idx as usize).copied() == Some(i));
            let style = if highlight {
                highlight_style
            } else {
                base_style
            };
            spans.push(Span::styled(ch.to_string(), style));
        }

        if !entry.secondary.is_empty() {
            let pad = name_col_width.saturating_sub(label_char_count);
            spans.push(Span::styled(
                format!("{:pad$}  {}", "", entry.secondary, pad = pad),
                dim_style,
            ));
        }

        if is_selected {
            paint_row_bg(frame, row_rect, palette.surface0);
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
    }
}

fn kind_tag(kind: PickerKind) -> &'static str {
    match kind {
        PickerKind::Workspace => "ws",
        PickerKind::Tab => "tab",
        PickerKind::Pane => "pane",
        PickerKind::Agent => "agent",
    }
}

fn paint_row_bg(frame: &mut Frame, rect: Rect, bg: ratatui::style::Color) {
    let buf = frame.buffer_mut();
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            buf[(x, y)].set_style(Style::default().bg(bg));
        }
    }
}

fn popup_rect(area: Rect) -> Option<Rect> {
    if area.width < 8 || area.height < 6 {
        return None;
    }
    let popup_w = ((area.width as u32 * 70) / 100) as u16;
    let popup_h = ((area.height as u32 * 70) / 100) as u16;
    let popup_w = popup_w.max(8).min(area.width.saturating_sub(2));
    let popup_h = popup_h.max(6).min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    Some(Rect::new(x, y, popup_w, popup_h))
}

fn paint_backdrop(frame: &mut Frame, area: Rect, palette: &crate::app::state::Palette) {
    let buf = frame.buffer_mut();
    let blank = {
        let mut cell = Cell::EMPTY;
        cell.set_style(Style::default().bg(palette.surface_dim));
        cell
    };
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf[(x, y)] = blank.clone();
        }
    }
}
