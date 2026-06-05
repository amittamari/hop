//! Pluggable result-list columns and the responsive layout solver. Pure logic:
//! produces resolved widths from column definitions; rendering lives in the TUI.

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
    /// Hard floor for the column. Non-flex columns can grow to fit visible
    /// content, but only flex columns absorb leftover width.
    pub min_width: u16,
    pub flex: bool,
}

/// The default v1 column set (directory intentionally absent).
pub fn default_columns() -> Vec<Column> {
    vec![
        Column {
            id: "agent",
            header: "AGENT",
            align: Align::Left,
            priority: u8::MAX,
            min_width: 6,
            flex: false,
        },
        Column {
            id: "repo",
            header: "REPO",
            align: Align::Left,
            priority: 10,
            min_width: 4,
            flex: false,
        },
        Column {
            id: "branch",
            header: "BRANCH",
            align: Align::Left,
            priority: 20,
            min_width: 6,
            flex: false,
        },
        Column {
            id: "title",
            header: "TITLE",
            align: Align::Left,
            priority: u8::MAX,
            min_width: 10,
            flex: true,
        },
        Column {
            id: "msgs",
            header: "MSGS",
            align: Align::Right,
            priority: 40,
            min_width: 4,
            flex: false,
        },
        Column {
            id: "pr",
            header: "PR",
            align: Align::Left,
            priority: 50,
            min_width: 5,
            flex: false,
        },
        Column {
            id: "time",
            header: "TIME",
            align: Align::Right,
            priority: 30,
            min_width: 4,
            flex: false,
        },
    ]
}

/// Apply user column preferences to a default column list.
///
/// Unknown ids are ignored. Ordered ids are emitted first, then the remaining
/// enabled columns keep their default relative order.
pub fn configured_columns(
    columns: Vec<Column>,
    disabled: &[String],
    order: &[String],
) -> Vec<Column> {
    let enabled = |id: &str| !disabled.iter().any(|d| d == id);
    let mut out = Vec::new();

    for id in order {
        if !enabled(id) || out.iter().any(|c: &Column| c.id == id) {
            continue;
        }
        if let Some(col) = columns.iter().find(|c| c.id == id) {
            out.push(col.clone());
        }
    }

    for col in columns {
        if enabled(col.id) && !out.iter().any(|c| c.id == col.id) {
            out.push(col);
        }
    }

    out
}

const GAP: u16 = 1;

/// Decide which columns are visible and their widths for a given pane width.
/// Drops columns by descending `priority` until the rest fit; TITLE always
/// survives and flexes to fill leftover space.
pub fn solve_layout(columns: &[Column], total_width: u16) -> Vec<(usize, u16)> {
    solve_layout_with_desired(columns, total_width, &[])
}

/// Like [`solve_layout`], but lets callers provide desired widths for visible
/// content. Non-flex columns grow toward those desired widths first; leftover
/// width goes to the flex column.
pub fn solve_layout_with_desired(
    columns: &[Column],
    total_width: u16,
    desired_widths: &[u16],
) -> Vec<(usize, u16)> {
    let mut kept: Vec<usize> = (0..columns.len()).collect();

    let floor = |i: usize| -> u16 {
        columns[i]
            .min_width
            .max(columns[i].header.chars().count() as u16)
    };

    let desired = |i: usize| -> u16 {
        desired_widths
            .get(i)
            .copied()
            .unwrap_or_else(|| floor(i))
            .max(floor(i))
    };

    let needed = |kept: &[usize]| -> u16 {
        let cols: u16 = kept.iter().map(|&i| floor(i)).sum();
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

    // Assign floors, then grow non-flex columns up to visible content. Any
    // remaining width is won by the flex column, which is the title by default.
    let used = needed(&kept);
    let mut extra = total_width.saturating_sub(used);
    let mut out: Vec<(usize, u16)> = kept.iter().map(|&i| (i, floor(i))).collect();

    for (i, width) in out.iter_mut() {
        if columns[*i].flex {
            continue;
        }
        let grow_by = desired(*i).saturating_sub(*width).min(extra);
        *width += grow_by;
        extra -= grow_by;
    }

    if extra > 0 {
        if let Some((i, width)) = out.iter_mut().find(|(i, _)| columns[*i].flex) {
            *width += extra;
            debug_assert!(columns[*i].flex);
        }
    }

    out
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
    fn volatile_columns_drop_before_repo_and_branch_when_narrow() {
        let cols = default_columns();
        // width that forces some drops but still has room for the context columns
        let layout = solve_layout(&cols, 38);
        let ids: Vec<&str> = layout.iter().map(|&(i, _)| cols[i].id).collect();
        assert!(ids.contains(&"repo"));
        assert!(ids.contains(&"branch"));
        assert!(ids.contains(&"title") && ids.contains(&"agent"));
        assert!(!ids.contains(&"pr"));
        assert!(!ids.contains(&"msgs"));
    }

    #[test]
    fn flex_column_absorbs_extra_width() {
        let cols = default_columns();
        let layout = solve_layout(&cols, 200);
        let title_w = layout
            .iter()
            .find(|&&(i, _)| cols[i].id == "title")
            .unwrap()
            .1;
        assert!(
            title_w > 12,
            "title should grow past its min on a wide pane"
        );
    }

    #[test]
    fn desired_non_flex_widths_grow_before_title_takes_leftover() {
        let cols = default_columns();
        let desired = vec![6, 3, 18, 60, 4, 5, 4];
        let layout = solve_layout_with_desired(&cols, 100, &desired);
        let width = |id| {
            layout
                .iter()
                .find(|&&(i, _)| cols[i].id == id)
                .map(|&(_, w)| w)
                .unwrap()
        };

        assert_eq!(width("repo"), 4); // header is wider than the visible value
        assert_eq!(width("branch"), 18);
        assert!(width("title") > cols.iter().find(|c| c.id == "title").unwrap().min_width);
    }

    #[test]
    fn fit_pads_and_truncates() {
        assert_eq!(fit("ab", 4, Align::Left), "ab  ");
        assert_eq!(fit("ab", 4, Align::Right), "  ab");
        assert_eq!(fit("abcdef", 4, Align::Left), "abc…");
    }

    #[test]
    fn configured_columns_orders_and_disables() {
        let disabled = vec!["pr".to_string()];
        let order = vec![
            "time".to_string(),
            "title".to_string(),
            "missing".to_string(),
        ];
        let cols = configured_columns(default_columns(), &disabled, &order);
        let ids: Vec<&str> = cols.iter().map(|c| c.id).collect();
        assert_eq!(ids[0..2], ["time", "title"]);
        assert!(!ids.contains(&"pr"));
        assert!(ids.contains(&"agent"));
    }
}
