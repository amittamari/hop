//! Pluggable result-list columns and the responsive layout solver. Pure logic:
//! produces per-row cell text and resolved widths; rendering lives in the TUI.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// A column definition. `flex` columns absorb leftover width (only TITLE).
#[derive(Debug, Clone)]
pub struct Column {
    pub id: &'static str,
    pub header: &'static str,
    pub align: Align,
    /// Higher drops first when the pane is narrow. `u8::MAX` = never drop.
    pub priority: u8,
    pub min_width: u16,
    pub flex: bool,
}

/// The default v1 column set (directory intentionally absent).
pub fn default_columns() -> Vec<Column> {
    vec![
        Column { id: "agent",  header: "",       align: Align::Left,  priority: u8::MAX, min_width: 6,  flex: false },
        Column { id: "repo",   header: "REPO",   align: Align::Left,  priority: 30,      min_width: 8,  flex: false },
        Column { id: "branch", header: "BRANCH", align: Align::Left,  priority: 40,      min_width: 10, flex: false },
        Column { id: "title",  header: "TITLE",  align: Align::Left,  priority: u8::MAX, min_width: 12, flex: true  },
        Column { id: "msgs",   header: "MSGS",   align: Align::Right, priority: 10,      min_width: 4,  flex: false },
        Column { id: "pr",     header: "PR",     align: Align::Left,  priority: 50,      min_width: 5,  flex: false },
        Column { id: "time",   header: "TIME",   align: Align::Right, priority: 20,      min_width: 4,  flex: false },
    ]
}

const GAP: u16 = 1;

/// Decide which columns are visible and their widths for a given pane width.
/// Drops columns by descending `priority` until the rest fit; TITLE always
/// survives and flexes to fill leftover space.
pub fn solve_layout(columns: &[Column], total_width: u16) -> Vec<(usize, u16)> {
    let mut kept: Vec<usize> = (0..columns.len()).collect();

    let needed = |kept: &[usize]| -> u16 {
        let cols: u16 = kept.iter().map(|&i| columns[i].min_width).sum();
        let gaps = (kept.len().saturating_sub(1)) as u16 * GAP;
        cols + gaps
    };

    // Drop highest-priority (largest number, != MAX) columns until it fits.
    while needed(&kept) > total_width && kept.len() > 1 {
        let drop = kept
            .iter()
            .copied()
            .filter(|&i| columns[i].priority != u8::MAX)
            .max_by_key(|&i| columns[i].priority);
        match drop {
            Some(i) => kept.retain(|&k| k != i),
            None => break, // only un-droppable columns remain
        }
    }

    // Assign widths: min_width to each, extra to the flex column.
    let used = needed(&kept);
    let extra = total_width.saturating_sub(used);
    kept.into_iter()
        .map(|i| {
            let w = if columns[i].flex {
                columns[i].min_width + extra
            } else {
                columns[i].min_width
            };
            (i, w)
        })
        .collect()
}

/// Pad/truncate `s` to exactly `width` columns per `align`.
pub fn fit(s: &str, width: u16, align: Align) -> String {
    let w = width as usize;
    let len = s.chars().count();
    if len == w {
        return s.to_string();
    }
    if len > w {
        if w == 0 {
            return String::new();
        }
        let keep = w.saturating_sub(1);
        let mut out: String = s.chars().take(keep).collect();
        out.push('…');
        return out;
    }
    let pad = " ".repeat(w - len);
    match align {
        Align::Left => format!("{s}{pad}"),
        Align::Right => format!("{pad}{s}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_always_survives_when_very_narrow() {
        let cols = default_columns();
        let layout = solve_layout(&cols, 14);
        let ids: Vec<&str> = layout.iter().map(|&(i, _)| cols[i].id).collect();
        assert!(ids.contains(&"title"));
        assert!(ids.contains(&"agent"));
    }

    #[test]
    fn pr_drops_before_repo_when_narrow() {
        let cols = default_columns();
        // width that forces some drops but not all
        let layout = solve_layout(&cols, 40);
        let ids: Vec<&str> = layout.iter().map(|&(i, _)| cols[i].id).collect();
        // pr (priority 50) drops before branch (40) / repo (30)
        if !ids.contains(&"repo") {
            assert!(!ids.contains(&"pr"));
        }
        assert!(ids.contains(&"title") && ids.contains(&"agent"));
    }

    #[test]
    fn flex_column_absorbs_extra_width() {
        let cols = default_columns();
        let layout = solve_layout(&cols, 200);
        let title_w = layout.iter().find(|&&(i, _)| cols[i].id == "title").unwrap().1;
        assert!(title_w > 12, "title should grow past its min on a wide pane");
    }

    #[test]
    fn fit_pads_and_truncates() {
        assert_eq!(fit("ab", 4, Align::Left), "ab  ");
        assert_eq!(fit("ab", 4, Align::Right), "  ab");
        assert_eq!(fit("abcdef", 4, Align::Left), "abc…");
    }
}
