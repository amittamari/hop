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
            .max(display_width(columns[i].header) as u16)
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
    if w == 0 {
        return String::new();
    }

    let mut len = 0usize;
    for c in s.chars() {
        let cw = char_width(c);
        if len + cw > w {
            let keep = w.saturating_sub(1);
            let mut out = take_display_width(s, keep);
            out.push('…');
            return out;
        }
        len += cw;
    }

    if len == w {
        return s.to_string();
    }
    let pad = " ".repeat(w - len);
    match align {
        Align::Left => format!("{s}{pad}"),
        Align::Right => format!("{pad}{s}"),
    }
}

pub fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Like `fit`, but truncates from the start: `…rsonal/hop`.
pub fn fit_end(s: &str, width: u16) -> String {
    let w = width as usize;
    if w == 0 {
        return String::new();
    }
    let total = display_width(s);
    if total <= w {
        return s.to_string();
    }
    let keep = w.saturating_sub(1);
    let skip = total - keep;
    let mut skipped = 0usize;
    let mut start_byte = 0;
    for (i, c) in s.char_indices() {
        if skipped >= skip {
            start_byte = i;
            break;
        }
        skipped += char_width(c);
        start_byte = i + c.len_utf8();
    }
    format!("…{}", &s[start_byte..])
}

fn take_display_width(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        let cw = char_width(c);
        if used + cw > width {
            break;
        }
        out.push(c);
        used += cw;
    }
    out
}

fn char_width(c: char) -> usize {
    let u = c as u32;
    if u == 0
        || u < 0x20
        || (0x7f..=0x9f).contains(&u)
        || (0x0300..=0x036f).contains(&u)
        || (0x0483..=0x0489).contains(&u)
        || (0x0591..=0x05bd).contains(&u)
        || u == 0x05bf
        || (0x05c1..=0x05c2).contains(&u)
        || (0x05c4..=0x05c5).contains(&u)
        || u == 0x05c7
        || (0x0610..=0x061a).contains(&u)
        || (0x064b..=0x065f).contains(&u)
        || u == 0x0670
        || (0x06d6..=0x06dc).contains(&u)
        || (0x06df..=0x06e4).contains(&u)
        || (0x06e7..=0x06e8).contains(&u)
        || (0x06ea..=0x06ed).contains(&u)
        || (0x0711..=0x0711).contains(&u)
        || (0x0730..=0x074a).contains(&u)
        || (0x07a6..=0x07b0).contains(&u)
        || (0x07eb..=0x07f3).contains(&u)
        || (0x0816..=0x0819).contains(&u)
        || (0x081b..=0x0823).contains(&u)
        || (0x0825..=0x0827).contains(&u)
        || (0x0829..=0x082d).contains(&u)
        || (0x0859..=0x085b).contains(&u)
        || (0x08d3..=0x08e1).contains(&u)
        || (0x08e3..=0x0902).contains(&u)
        || (0x093a..=0x093a).contains(&u)
        || u == 0x093c
        || (0x0941..=0x0948).contains(&u)
        || u == 0x094d
        || (0x0951..=0x0957).contains(&u)
        || (0x0962..=0x0963).contains(&u)
        || (0x0981..=0x0981).contains(&u)
        || u == 0x09bc
        || (0x09c1..=0x09c4).contains(&u)
        || u == 0x09cd
        || u == 0x09e2
        || u == 0x09e3
        || u == 0x0a01
        || u == 0x0a02
        || u == 0x0a3c
        || u == 0x0a41
        || u == 0x0a42
        || u == 0x0a47
        || u == 0x0a48
        || u == 0x0a4b
        || u == 0x0a4c
        || u == 0x0a4d
        || u == 0x0a51
        || u == 0x0a70
        || u == 0x0a71
        || u == 0x0a75
        || (0x0a81..=0x0a82).contains(&u)
        || u == 0x0abc
        || (0x0ac1..=0x0ac5).contains(&u)
        || (0x0ac7..=0x0ac8).contains(&u)
        || u == 0x0acd
        || (0x0ae2..=0x0ae3).contains(&u)
        || u == 0x200b
        || u == 0x200c
        || u == 0x200d
        || (0xfe00..=0xfe0f).contains(&u)
    {
        0
    } else if (0x1100..=0x115f).contains(&u)
        || u == 0x2329
        || u == 0x232a
        || (0x2e80..=0xa4cf).contains(&u)
        || (0xac00..=0xd7a3).contains(&u)
        || (0xf900..=0xfaff).contains(&u)
        || (0xfe10..=0xfe19).contains(&u)
        || (0xfe30..=0xfe6f).contains(&u)
        || (0xff00..=0xff60).contains(&u)
        || (0xffe0..=0xffe6).contains(&u)
        || (0x1f300..=0x1f64f).contains(&u)
        || (0x1f900..=0x1f9ff).contains(&u)
        || (0x20000..=0x3fffd).contains(&u)
    {
        2
    } else {
        1
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
    fn fit_uses_terminal_display_width_for_wide_glyphs() {
        assert_eq!(display_width("中x"), 3);
        assert_eq!(fit("中x", 3, Align::Left), "中x");
        assert_eq!(fit("中x", 2, Align::Left), "…");
        assert_eq!(fit("a中b", 4, Align::Left), "a中b");
    }

    #[test]
    fn fit_keeps_combining_marks_with_zero_width() {
        let cafe = "cafe\u{0301}";
        assert_eq!(display_width(cafe), 4);
        assert_eq!(fit(cafe, 4, Align::Left), cafe);
        assert_eq!(fit(cafe, 3, Align::Left), "ca…");
    }

    #[test]
    fn fit_end_truncates_start() {
        assert_eq!(fit_end("/Users/amitt/workspaces/personal/hop", 20), "…spaces/personal/hop");
        assert_eq!(fit_end("/short", 20), "/short");
        assert_eq!(fit_end("abc", 3), "abc");
        assert_eq!(fit_end("abcd", 3), "…cd");
        assert_eq!(fit_end("x", 0), "");
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
