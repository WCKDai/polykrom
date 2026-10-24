//! Following an agent (#78): what an agent does over the control channel or MCP, shown in the
//! app, so a person can follow along as it builds and inspects the model. Each command the agent
//! runs (`engine.execute`, and each step of `engine.batch`) is listed in words in the viewport's
//! bottom-right corner with its outcome, the newest last, and fades when the agent has been quiet a while. The
//! result of an inspection it runs is drawn on the model until the next one, or for
//! [`MARK_SECONDS`]: overlaps and collisions as their boxes, thread engagements as theirs in
//! another colour, and pairs closer than the clearance as a line between their nearest points
//! with the distance. A person's own commands are not listed: they see those already.

use std::collections::VecDeque;

use egui::{Align2, Color32, FontId, Pos2, Stroke, pos2, vec2};
use serde_json::Value;
use solidcraft_engine::geom::Vec3;

use crate::SolveApp;
use crate::i18n::tr;
use crate::theme::Tokens;

/// Most commands listed.
const SHOWN: usize = 6;
/// Seconds an entry stays at full strength after the agent's last command, then fades out over
/// as long again.
const HOLD_SECONDS: f64 = 30.0;
/// Seconds an inspection's marks stay on the model.
const MARK_SECONDS: f64 = 45.0;
/// Most marks one inspection draws (the first ones of its lists).
const MAX_MARKS: usize = 40;

/// One command the agent ran.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// When it ran (the app's clock, `SolveApp::now`).
    pub at: f64,
    /// The command's name as the toolbar shows it.
    pub label: String,
    /// What it came to, in a few words.
    pub outcome: String,
    pub ok: bool,
}

/// What an inspection marks on the model.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// A box: an overlap, a collision or a thread engagement.
    Region { min: Vec3, max: Vec3, kind: Region },
    /// Two bodies closer than the clearance: their nearest points and the distance (mm).
    Gap { from: Vec3, to: Vec3, mm: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Region {
    Overlap,
    Thread,
}

/// The agent's commands and the marks of its last inspection.
#[derive(Default)]
pub struct Feed {
    pub entries: VecDeque<Entry>,
    pub marks: Vec<Mark>,
    /// When the marks were made.
    pub marks_at: f64,
}

/// Note a command the agent ran, and keep what an inspection found to draw on the model.
pub fn record(app: &mut SolveApp, id: &str, result: &Result<Value, String>) {
    let label = solidcraft_engine::command_specs().into_iter().find(|s| s.id == id).map_or(id.to_string(), |s| tr(s.label).to_string());
    let (outcome, ok) = match result {
        Ok(v) => (outcome(id, v), true),
        Err(e) => (tr("ui.agent_outcome_failed").replace("{error}", e.lines().next().unwrap_or(e)), false),
    };
    let feed = &mut app.agent_feed;
    feed.entries.push_back(Entry { at: app.now, label, outcome, ok });
    while feed.entries.len() > SHOWN {
        feed.entries.pop_front();
    }
    if let Ok(v) = result
        && let Some(marks) = marks(id, v)
    {
        feed.marks = marks;
        feed.marks_at = app.now;
    }
}

/// A command's outcome in words: what an inspection found, else "done".
pub fn outcome(id: &str, v: &Value) -> String {
    let count = |key: &str| v.get(key).and_then(Value::as_array).map_or(0, Vec::len);
    // One of a thing reads "1 overlap", more "2 overlaps" (`_one` keys).
    let n = |key: &str, n: usize| {
        let one = format!("{key}_one");
        let text = if n == 1 && tr(&one) != one { tr(&one) } else { tr(key) };
        text.replace("{count}", &n.to_string())
    };
    let num = |key: &str| v.get(key).and_then(Value::as_f64);
    match id {
        "inspect.clearance" => {
            let mut parts = Vec::new();
            for (list, key) in
                [("overlaps", "ui.agent_outcome_overlaps"), ("threaded", "ui.agent_outcome_threaded"), ("near", "ui.agent_outcome_near")]
            {
                if count(list) > 0 {
                    parts.push(n(key, count(list)));
                }
            }
            if parts.is_empty() { tr("ui.agent_outcome_clear").to_string() } else { parts.join(", ") }
        }
        "inspect.interference" => {
            let hits = num("count").map_or(count("interferences"), |c| c as usize);
            if hits == 0 && count("threaded") == 0 {
                tr("ui.agent_outcome_clear").to_string()
            } else {
                let mut parts = vec![n("ui.agent_outcome_overlaps", hits)];
                if count("threaded") > 0 {
                    parts.push(n("ui.agent_outcome_threaded", count("threaded")));
                }
                parts.join(", ")
            }
        }
        "motion.interference" => {
            if v.get("clear").and_then(Value::as_bool) == Some(true) {
                tr("ui.agent_outcome_clear").to_string()
            } else {
                n("ui.agent_outcome_collisions", count("collisions"))
            }
        }
        "joint.sweep" => match num("stopped_at") {
            Some(at) => tr("ui.agent_outcome_stops").replace("{value}", &format!("{at:.2}")),
            None => tr("ui.agent_outcome_clear").to_string(),
        },
        "inspect.measure" => {
            let total = v.get("total");
            match (total.and_then(|t| t.get("volume_mm3")).and_then(Value::as_f64), total.and_then(|t| t.get("mass_g")).and_then(Value::as_f64)) {
                (Some(vol), Some(mass)) => {
                    tr("ui.agent_outcome_measured").replace("{volume}", &format!("{vol:.0}")).replace("{mass}", &format!("{mass:.1}"))
                }
                _ => tr("ui.agent_outcome_done").to_string(),
            }
        }
        _ => tr("ui.agent_outcome_done").to_string(),
    }
}

/// The marks an inspection's result draws on the model (None: the command is no inspection, and
/// the marks of the last one stay).
pub fn marks(id: &str, v: &Value) -> Option<Vec<Mark>> {
    let point = |p: Option<&Value>| -> Option<Vec3> {
        let a = p?.as_array()?;
        Some(Vec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?))
    };
    let region = |o: &Value, kind: Region| -> Option<Mark> {
        let b = o.get("bbox")?;
        Some(Mark::Region { min: point(b.get("min"))?, max: point(b.get("max"))?, kind })
    };
    let list = |key: &str| v.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out: Vec<Mark> = match id {
        "inspect.clearance" => {
            let mut m: Vec<Mark> = list("overlaps").iter().filter_map(|o| region(o, Region::Overlap)).collect();
            m.extend(
                list("near")
                    .iter()
                    .filter_map(|o| Some(Mark::Gap { from: point(o.get("from"))?, to: point(o.get("to"))?, mm: o.get("distance_mm")?.as_f64()? })),
            );
            // A thread engagement has no box of its own in the reply: it is marked by its overlap's
            // box in inspect.interference only.
            m
        }
        "inspect.interference" => list("interferences").iter().filter_map(|o| region(o, Region::Overlap)).collect(),
        _ => return None,
    };
    out.truncate(MAX_MARKS);
    Some(out)
}

/// How strongly the feed shows `age` seconds after the agent's last command: full, then fading.
fn strength(age: f64) -> f32 {
    (1.0 - ((age - HOLD_SECONDS) / HOLD_SECONDS).clamp(0.0, 1.0)) as f32
}

/// Draw the feed in the viewport's corner and the last inspection's marks on the model.
pub fn paint(app: &SolveApp, ctx: &egui::Context) {
    let Some(rect) = app.viewport.rect else { return };
    let feed = &app.agent_feed;
    let t = Tokens::get();
    // Over the model but under every window and dialog.
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("sc_agent_activity"))).with_clip_rect(rect);
    if !feed.marks.is_empty() && app.now - feed.marks_at < MARK_SECONDS {
        let proj = crate::viewport::projection(app, rect);
        let fade = (1.0 - ((app.now - feed.marks_at) / MARK_SECONDS) as f32).clamp(0.2, 1.0);
        for mark in &feed.marks {
            match mark {
                Mark::Region { min, max, kind } => {
                    let colour = match kind {
                        Region::Overlap => t.interference,
                        Region::Thread => t.agent_ok,
                    };
                    let corner = |i: usize| {
                        Vec3::new(
                            if i & 1 == 0 { min.x } else { max.x },
                            if i & 2 == 0 { min.y } else { max.y },
                            if i & 4 == 0 { min.z } else { max.z },
                        )
                    };
                    for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                        if let (Some(p), Some(q)) = (proj.to_screen(corner(a)), proj.to_screen(corner(b))) {
                            painter.line_segment([p, q], Stroke::new(2.0, colour.gamma_multiply(fade)));
                        }
                    }
                }
                Mark::Gap { from, to, mm } => {
                    if let (Some(p), Some(q)) = (proj.to_screen(*from), proj.to_screen(*to)) {
                        painter.line_segment([p, q], Stroke::new(2.0, t.accent.gamma_multiply(fade)));
                        for end in [p, q] {
                            painter.circle_filled(end, 3.0, t.accent.gamma_multiply(fade));
                        }
                        let mid = pos2((p.x + q.x) / 2.0, (p.y + q.y) / 2.0 - 10.0);
                        painter.text(mid, Align2::CENTER_BOTTOM, format!("{mm:.3} mm"), FontId::proportional(t.font_small), t.text.gamma_multiply(fade));
                    }
                }
            }
        }
    }
    let Some(last) = feed.entries.back() else { return };
    let k = strength(app.now - last.at);
    if k <= 0.0 {
        return;
    }
    let font = FontId::proportional(t.font_label);
    let line = 18.0;
    let rows = feed.entries.len() as f32 + 1.0;
    // In the viewport's bottom-right corner: the browser floats over its top-left, the view cube
    // sits top-right and the navigation bar bottom-centre.
    let width = 320.0_f32.min(rect.width() - 24.0).max(120.0);
    let size = vec2(width, rows * line + 12.0);
    let at = rect.right_bottom() - size - vec2(12.0, 64.0);
    let frame = egui::Rect::from_min_size(at, size);
    painter.rect_filled(frame, 6.0, t.panel.gamma_multiply(0.92 * k));
    painter.rect_stroke(frame, 6.0, Stroke::new(1.0, t.border.gamma_multiply(k)), egui::StrokeKind::Inside);
    let mut y = at.y + 6.0;
    let title = pos2(at.x + 10.0, y);
    painter.circle_filled(title + vec2(4.0, line / 2.0), 4.0, crate::icons::pointer::agent().gamma_multiply(k));
    painter.text(title + vec2(14.0, line / 2.0), Align2::LEFT_CENTER, tr("ui.agent_activity_title"), font.clone(), t.text.gamma_multiply(k));
    y += line;
    for e in &feed.entries {
        let colour = if e.ok { t.text } else { t.interference };
        let row: Pos2 = pos2(at.x + 10.0, y + line / 2.0);
        let text = format!("{} · {}", e.label, e.outcome);
        let galley = painter.layout(text, font.clone(), colour.gamma_multiply(k), width - 20.0);
        painter.galley(row - vec2(0.0, galley.size().y / 2.0), galley, Color32::TRANSPARENT);
        y += line;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inspections_are_told_by_what_they_found() {
        let clear = json!({"overlaps": [], "near": [], "threaded": []});
        assert_eq!(outcome("inspect.clearance", &clear), tr("ui.agent_outcome_clear"));
        let found = json!({"overlaps": [{}, {}], "threaded": [{}], "near": []});
        let said = outcome("inspect.clearance", &found);
        assert!(said.contains('2') && said.contains('1'), "{said}");
        // One of a thing is singular.
        assert_eq!(outcome("inspect.clearance", &json!({"overlaps": [{}]})), tr("ui.agent_outcome_overlaps_one").replace("{count}", "1"));
        assert_eq!(outcome("motion.interference", &json!({"clear": true})), tr("ui.agent_outcome_clear"));
        assert!(outcome("motion.interference", &json!({"clear": false, "collisions": [{}, {}, {}]})).contains('3'));
        assert!(outcome("joint.sweep", &json!({"stopped_at": 33.137})).contains("33.14"));
        assert!(outcome("inspect.measure", &json!({"total": {"volume_mm3": 6000.0, "mass_g": 47.1}})).contains("6000"));
        assert_eq!(outcome("solid.extrude", &json!({"feature": 3})), tr("ui.agent_outcome_done"));
    }

    #[test]
    fn an_inspection_marks_its_overlaps_and_gaps() {
        let v = json!({
            "overlaps": [{"a": "A", "b": "B", "bbox": {"min": [0, 0, 0], "max": [1, 2, 3]}}],
            "near": [{"a": "A", "b": "C", "distance_mm": 0.1, "from": [5, 0, 0], "to": [5.1, 0, 0]}],
        });
        let m = marks("inspect.clearance", &v).unwrap_or_default();
        assert_eq!(m.len(), 2);
        assert!(matches!(m[0], Mark::Region { kind: Region::Overlap, .. }));
        assert!(matches!(m[1], Mark::Gap { mm, .. } if (mm - 0.1).abs() < 1e-12));
        // A modelling command keeps the last inspection's marks.
        assert!(marks("solid.extrude", &json!({})).is_none());
    }

    #[test]
    fn the_feed_holds_then_fades() {
        assert_eq!(strength(0.0), 1.0);
        assert_eq!(strength(HOLD_SECONDS), 1.0);
        assert!((strength(HOLD_SECONDS * 1.5) - 0.5).abs() < 1e-6);
        assert_eq!(strength(HOLD_SECONDS * 2.0), 0.0);
    }
}
