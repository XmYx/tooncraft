//! `cargo xtask parity`: scores `docs/target-app-parity.md`.
//!
//! The map is a list of areas (`## ` headings), each with a `Weight: <n>` line and one or more
//! feature tables, recognised by their `| Feature | Refs | Status | Size | Notes |` header.
//!
//! - Status: `D` done, `X` exceeds (both count 1), `P` partial (½), `M` missing (0), `O` out of
//!   scope (left out of every denominator; the notes must give the reason).
//! - Size is the effort to reach parity on the row: `S` = 1, `M` = 2, `L` = 4, `XL` = 8.
//!
//! Three numbers come out:
//! - **row parity**: (D + X + ½·P) / in-scope rows, every row counting the same;
//! - **area effort parity**: the same sum with each row weighted by its size, inside one area;
//! - **weighted parity**: the area effort parities averaged with the area weights.
//!
//! Parsing is strict on purpose: inside a feature table every row needs exactly the five cells, a
//! known status and size, and a feature name not used before in its area; a table outside an area
//! or an area with rows but no weight is an error. Every error names the line, so a malformed row
//! never scores silently.

use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The header every feature table starts with.
const HEADER: [&str; 5] = ["Feature", "Refs", "Status", "Size", "Notes"];

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Area {
    pub name: String,
    pub weight: f64,
    /// Rows by status: done, exceeds, partial, missing, out of scope.
    pub d: u32,
    pub x: u32,
    pub p: u32,
    pub m: u32,
    pub o: u32,
    /// Size-weighted credit and in-scope size total.
    pub credit: f64,
    pub size: f64,
}

impl Area {
    /// In-scope rows.
    pub fn rows(&self) -> u32 {
        self.d + self.x + self.p + self.m
    }
    pub fn row_score(&self) -> f64 {
        (f64::from(self.d + self.x) + 0.5 * f64::from(self.p)) / f64::from(self.rows().max(1)) * 100.0
    }
    pub fn effort_score(&self) -> f64 {
        if self.size > 0.0 { self.credit / self.size * 100.0 } else { 0.0 }
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub areas: Vec<Area>,
}

impl Report {
    fn total(&self) -> Area {
        let mut t = Area::default();
        for a in &self.areas {
            t.d += a.d;
            t.x += a.x;
            t.p += a.p;
            t.m += a.m;
            t.o += a.o;
            t.credit += a.credit;
            t.size += a.size;
        }
        t
    }

    /// Area effort parities averaged with the area weights.
    pub fn weighted(&self) -> f64 {
        let w: f64 = self.areas.iter().map(|a| a.weight).sum();
        if w <= 0.0 {
            return 0.0;
        }
        self.areas.iter().map(|a| a.weight * a.effort_score()).sum::<f64>() / w
    }

    /// Row parity over every area.
    #[cfg(test)]
    pub fn row(&self) -> f64 {
        self.total().row_score()
    }

    /// The Markdown block written between the `parity:` markers.
    pub fn block(&self) -> String {
        let t = self.total();
        let wsum: f64 = self.areas.iter().map(|a| a.weight).sum();
        let mut s = String::from("| | Value | Kind |\n|---|---|---|\n");
        let _ = writeln!(s, "| In-scope feature rows | {} | counted |", t.rows());
        let _ = writeln!(s, "| Done / exceeds / partial / missing | {} / {} / {} / {} | counted |", t.d, t.x, t.p, t.m);
        let _ = writeln!(s, "| Out of scope (not in any denominator) | {} | counted |", t.o);
        let _ = writeln!(s, "| **Row parity**, (D + X + ½·P) / rows | **{:.1} %** | counted |", t.row_score());
        let _ = writeln!(s, "| **Weighted parity**, area weights × size-weighted rows | **{:.1} %** | counted |", self.weighted());
        let _ = writeln!(s, "| Size-weighted parity over all rows, no area weights | {:.1} % | counted |", t.effort_score());
        s.push('\n');
        s.push_str("| Area | Weight | Rows | D | X | P | M | Out | Row parity | Effort parity |\n");
        s.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
        for a in &self.areas {
            let share = if wsum > 0.0 { a.weight / wsum * 100.0 } else { 0.0 };
            let _ = writeln!(
                s,
                "| {} | {} ({:.0} %) | {} | {} | {} | {} | {} | {} | {:.0} % | {:.0} % |",
                a.name,
                a.weight,
                share,
                a.rows(),
                a.d,
                a.x,
                a.p,
                a.m,
                a.o,
                a.row_score(),
                a.effort_score()
            );
        }
        s
    }
}

fn size_of(s: &str) -> Option<f64> {
    match s {
        "S" => Some(1.0),
        "M" => Some(2.0),
        "L" => Some(4.0),
        "XL" => Some(8.0),
        _ => None,
    }
}

/// The cells of a Markdown table line, without the empty strings outside the outer pipes.
fn cells(line: &str) -> Vec<&str> {
    let inner = line.trim().strip_prefix('|').unwrap_or(line);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(str::trim).collect()
}

/// Parses the map.
pub fn score(text: &str) -> Result<Report, String> {
    let mut report = Report::default();
    let mut current: Option<(Area, BTreeSet<String>)> = None;
    let mut in_block = false;
    let mut in_table = false;
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        // The generated block is output, not input.
        if line.starts_with("<!-- parity:start") {
            in_block = true;
            continue;
        }
        if line.starts_with("<!-- parity:end") {
            in_block = false;
            continue;
        }
        if in_block {
            continue;
        }
        if let Some(h) = line.strip_prefix("## ") {
            if let Some((a, _)) = current.take() {
                finish(&mut report, a)?;
            }
            current = Some((Area { name: h.trim().to_string(), ..Area::default() }, BTreeSet::new()));
            in_table = false;
            continue;
        }
        if let Some(w) = line.strip_prefix("Weight:") {
            let w = w.trim().trim_matches('*').trim();
            let w: f64 = w.parse().map_err(|_| format!("line {n}: weight {w:?} is not a number"))?;
            if !(w > 0.0 && w <= 100.0) {
                return Err(format!("line {n}: weight {w} must be in (0, 100]"));
            }
            match current.as_mut() {
                Some((a, _)) if a.weight > 0.0 => return Err(format!("line {n}: area {:?} has two weights", a.name)),
                Some((a, _)) => a.weight = w,
                None => return Err(format!("line {n}: weight outside an area")),
            }
            continue;
        }
        if !line.starts_with('|') {
            in_table = false;
            continue;
        }
        let cols = cells(line);
        if cols.first() == Some(&"Feature") {
            if cols != HEADER {
                return Err(format!("line {n}: a feature table header must be `| {} |`", HEADER.join(" | ")));
            }
            if current.is_none() {
                return Err(format!("line {n}: feature table outside an area"));
            }
            in_table = true;
            continue;
        }
        if !in_table || cols.first().is_some_and(|c| c.starts_with("---")) {
            continue;
        }
        if cols.len() != HEADER.len() {
            return Err(format!("line {n}: a feature row needs exactly {} cells, found {}", HEADER.len(), cols.len()));
        }
        let Some((a, names)) = current.as_mut() else { return Err(format!("line {n}: feature row outside an area")) };
        let (feature, refs, status, size, notes) = (cols[0], cols[1], cols[2], cols[3], cols[4]);
        if feature.is_empty() || refs.is_empty() {
            return Err(format!("line {n}: a feature row needs a feature name and its references"));
        }
        if !names.insert(feature.to_string()) {
            return Err(format!("line {n}: feature {feature:?} is listed twice in area {:?}", a.name));
        }
        let size = size_of(size).ok_or_else(|| format!("line {n}: size {size:?} must be S, M, L or XL"))?;
        let credit = match status {
            "D" => {
                a.d += 1;
                1.0
            }
            "X" => {
                a.x += 1;
                1.0
            }
            "P" => {
                a.p += 1;
                0.5
            }
            "M" => {
                a.m += 1;
                0.0
            }
            "O" => {
                if notes.is_empty() {
                    return Err(format!("line {n}: an out-of-scope row needs its reason in the notes"));
                }
                a.o += 1;
                continue;
            }
            other => return Err(format!("line {n}: status {other:?} must be D, X, P, M or O")),
        };
        a.credit += credit * size;
        a.size += size;
    }
    if let Some((a, _)) = current.take() {
        finish(&mut report, a)?;
    }
    if report.areas.is_empty() {
        return Err("no feature rows found in docs/target-app-parity.md".into());
    }
    Ok(report)
}

fn finish(report: &mut Report, a: Area) -> Result<(), String> {
    if a.rows() + a.o == 0 {
        return Ok(());
    }
    if a.weight <= 0.0 {
        return Err(format!("area {:?} has rows but no `Weight:` line", a.name));
    }
    if a.rows() == 0 {
        return Err(format!("area {:?} has only out-of-scope rows", a.name));
    }
    report.areas.push(a);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::score;

    const H: &str = "| Feature | Refs | Status | Size | Notes |\n|---|---|---|---|---|\n";

    fn map() -> String {
        format!(
            "# Map\n\n<!-- parity:start -->\n| A | 1 | D | S | x |\n<!-- parity:end -->\n\n## Intro\n\n| Code | Meaning |\n|---|---|\n| D | done |\n\n## One\n\nWeight: 3\n\n{H}| a | TV | D | S | |\n| b | TB | P | L | |\n| c | CS | M | M | |\n| d | PD | O | XL | vendor cloud |\n\n## Two\n\nWeight: **1**\n\n{H}| e | — | X | M | |\n"
        )
    }

    #[test]
    fn scores_rows_sizes_and_weights() {
        let r = score(&map()).unwrap();
        assert_eq!(r.areas.len(), 2, "areas without feature rows are skipped");
        let one = &r.areas[0];
        assert_eq!((one.d, one.p, one.m, one.o), (1, 1, 1, 1));
        assert!((one.row_score() - 50.0).abs() < 1e-9);
        // (1·1 + 0.5·4 + 0·2) / 7
        assert!((one.effort_score() - 300.0 / 7.0).abs() < 1e-9);
        assert!((r.areas[1].effort_score() - 100.0).abs() < 1e-9);
        let w = (3.0 * 300.0 / 7.0 + 100.0) / 4.0;
        assert!((r.weighted() - w).abs() < 1e-9);
        // Four in-scope rows: D, P, M, X.
        assert!((r.row() - 62.5).abs() < 1e-9);
    }

    #[test]
    fn out_of_scope_rows_leave_every_denominator() {
        let with = score(&map()).unwrap();
        let without = score(&map().replace("| d | PD | O | XL | vendor cloud |\n", "")).unwrap();
        assert_eq!(with.weighted(), without.weighted());
        assert_eq!(with.row(), without.row());
    }

    #[test]
    fn rejects_malformed_rows() {
        let bad = |m: String, why: &str| assert!(score(&m).is_err(), "{why}");
        bad(map().replace("| b | TB | P | L | |", "| b | TB | Q | L | |"), "bad status");
        bad(map().replace("| b | TB | P | L | |", "| b | TB | P | huge | |"), "bad size");
        bad(map().replace("| b | TB | P | L | |", "| b | TB | P |"), "missing cells");
        bad(map().replace("| b | TB | P | L | |", "| b | TB | P | L | | extra |"), "extra cells");
        bad(map().replace("| b | TB | P | L | |", "| a | TB | P | L | |"), "duplicate feature in an area");
        bad(map().replace("| b | TB | P | L | |", "|  | TB | P | L | |"), "no feature name");
        bad(map().replace("vendor cloud", ""), "out of scope without a reason");
        bad(map().replace("Weight: 3\n", ""), "no weight");
        bad(map().replace("Weight: 3", "Weight: 3\nWeight: 2"), "two weights");
        bad(map().replace("Weight: 3", "Weight: x"), "weight not a number");
        bad(map().replace("Weight: 3", "Weight: 0"), "zero weight");
        bad(
            map().replace(
                "| Feature | Refs | Status | Size | Notes |\n|---|---|---|---|---|\n| e",
                "| Feature | Refs | Status | Notes |\n|---|---|---|---|\n| e",
            ),
            "old header",
        );
        bad(format!("{H}| a | TV | D | S | |\n"), "table outside an area");
        bad(format!("## A\n\nWeight: 1\n\n{H}| d | PD | O | XL | reason |\n"), "only out-of-scope rows");
        bad("## One\n\nno table here\n".into(), "no rows");
    }

    #[test]
    fn the_same_feature_may_appear_in_two_areas() {
        let m = map().replace("| e | — | X | M | |", "| a | — | X | M | |");
        assert!(score(&m).is_ok());
    }

    #[test]
    fn block_is_stable() {
        let r = score(&map()).unwrap();
        let b = r.block();
        assert!(b.contains("| One | 3 (75 %) | 3 | 1 | 0 | 1 | 1 | 1 | 50 % | 43 % |"), "{b}");
        assert!(b.contains("| **Weighted parity**, area weights × size-weighted rows | **57.1 %** | counted |"), "{b}");
        assert_eq!(b, score(&map()).unwrap().block());
    }

    #[test]
    fn the_map_in_the_repository_scores() {
        let text = include_str!("../../docs/target-app-parity.md");
        let r = score(text).unwrap();
        assert!(r.areas.len() >= 18, "{} areas", r.areas.len());
        assert!(r.weighted() > 0.0 && r.weighted() <= 100.0);
    }
}
