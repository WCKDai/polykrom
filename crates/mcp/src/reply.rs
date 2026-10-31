//! Replies sized for agents (#266).
//!
//! An agent's client takes a reply of about 25k tokens; past that it spills the reply to a file
//! and the agent spends turns reading it back. Every tool's reply is therefore held to a budget
//! (`reply_kb`, default [`DEFAULT_KB`]). A reply over it keeps its shape, but its longest lists are
//! cut to their first items, with each list's `total` and how to ask for the rest under
//! `truncated`. One list can be paged (`page_path`, `page_offset`, `page_limit`), and `select`
//! (on `execute` and `batch` steps) keeps only the parts asked for.

use std::cell::Cell;

use serde_json::{Map, Value, json};

/// A reply's budget unless the call says otherwise, in kilobytes of reply text.
pub(crate) const DEFAULT_KB: f64 = 60.0;
/// The most a call may ask for: about what an agent's client takes.
const MAX_KB: f64 = 512.0;

/// How the reply to the call running on this thread is shaped.
#[derive(Clone, Debug, Default)]
pub(crate) struct Shape {
    kb: Option<f64>,
    page: Option<Page>,
}

#[derive(Clone, Debug, Default)]
struct Page {
    /// Dotted path to the list (`None`: the reply's longest list).
    path: Option<String>,
    offset: usize,
    limit: Option<usize>,
}

thread_local! {
    static SHAPE: std::cell::RefCell<Shape> = Default::default();
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// The schema entries of [`ARGS`], added to every tool.
pub(crate) fn schema() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("reply_kb".into(), json!({"type": "number", "description": format!("Most this reply may take, in KB (default {DEFAULT_KB}, at most {MAX_KB}). Past it, long lists are cut to their first items and `truncated` says how many each has and how to get the rest")}));
    m.insert("page_path".into(), json!({"type": "string", "description": "A list in the reply to page through, as a dotted path (e.g. `timeline`, `bodies`); default: its longest list"}));
    m.insert("page_offset".into(), json!({"type": "integer", "minimum": 0, "description": "The first item of that list to return (default 0)"}));
    m.insert(
        "page_limit".into(),
        json!({"type": "integer", "minimum": 1, "description": "How many items of that list to return; the reply's `page` gives the list's total"}),
    );
    m
}

/// Take the reply arguments out of a tool call's arguments.
pub(crate) fn take(a: &mut Map<String, Value>) -> Shape {
    let kb = a.remove("reply_kb").and_then(|v| v.as_f64()).filter(|k| k.is_finite() && *k > 0.0).map(|k| k.min(MAX_KB));
    let path = a.remove("page_path").and_then(|v| v.as_str().map(str::to_string)).filter(|p| !p.is_empty());
    let offset = a.remove("page_offset").and_then(|v| v.as_u64());
    let limit = a.remove("page_limit").and_then(|v| v.as_u64());
    let page = (path.is_some() || offset.is_some() || limit.is_some()).then(|| Page {
        path,
        offset: offset.unwrap_or(0) as usize,
        limit: limit.map(|l| l.max(1) as usize),
    });
    Shape { kb, page }
}

/// Run `f` with `shape` applying to the replies it makes on this thread.
pub(crate) fn shaped<R>(shape: Shape, f: impl FnOnce() -> R) -> R {
    let previous = SHAPE.with(|s| s.replace(shape));
    let was = ACTIVE.with(|a| a.replace(true));
    struct Restore(Option<Shape>, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(s) = self.0.take() {
                SHAPE.with(|c| *c.borrow_mut() = s);
            }
            ACTIVE.with(|a| a.set(self.1));
        }
    }
    let _restore = Restore(Some(previous), was);
    f()
}

/// `v`, paged and held to the call's budget.
pub(crate) fn fit(v: Value) -> Value {
    let shape = SHAPE.with(|s| s.borrow().clone());
    let mut v = v;
    if let Some(page) = &shape.page {
        v = paged(v, page);
    }
    held(v, shape.kb.unwrap_or(DEFAULT_KB))
}

fn size(v: &Value) -> usize {
    serde_json::to_string_pretty(v).map_or(0, |s| s.len())
}

/// The arrays in `v` with their dotted paths (list items as `name.N`).
fn lists(v: &Value, path: &str, out: &mut Vec<(String, usize, usize)>) {
    match v {
        Value::Array(a) => {
            out.push((path.to_string(), a.len(), size(v)));
            for (i, x) in a.iter().enumerate() {
                lists(x, &join(path, &i.to_string()), out);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                lists(x, &join(path, k), out);
            }
        }
        _ => {}
    }
}

fn join(path: &str, k: &str) -> String {
    if path.is_empty() { k.to_string() } else { format!("{path}.{k}") }
}

fn at<'a>(v: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    if path.is_empty() {
        return Some(v);
    }
    path.split('.').try_fold(v, |v, k| match v {
        Value::Object(o) => o.get_mut(k),
        Value::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
        _ => None,
    })
}

fn paged(mut v: Value, page: &Page) -> Value {
    let path = match &page.path {
        Some(p) => p.clone(),
        None => {
            let mut all = Vec::new();
            lists(&v, "", &mut all);
            match all.into_iter().max_by_key(|(_, n, _)| *n) {
                Some((p, _, _)) => p,
                None => return v,
            }
        }
    };
    let Some(Value::Array(list)) = at(&mut v, &path) else {
        if let Some(o) = v.as_object_mut() {
            o.insert("page".into(), json!({"path": path, "error": "no list there"}));
        }
        return v;
    };
    let total = list.len();
    let start = page.offset.min(total);
    let end = page.limit.map_or(total, |l| start.saturating_add(l).min(total));
    *list = list.drain(start..end).collect();
    let info = json!({"path": path, "offset": start, "returned": end - start, "total": total, "more": end < total});
    match v {
        Value::Object(mut o) => {
            o.insert("page".into(), info);
            Value::Object(o)
        }
        other => json!({"value": other, "page": info}),
    }
}

/// `v` cut down to `kb`: the longest lists first, halved until it fits, each one's total kept.
fn held(v: Value, kb: f64) -> Value {
    let budget = (kb * 1024.0) as usize;
    let before = size(&v);
    if before <= budget {
        return v;
    }
    let mut v = match v {
        Value::Object(_) => v,
        other => json!({"value": other}),
    };
    let mut cut: Vec<(String, usize, usize)> = Vec::new();
    for _ in 0..200 {
        if size(&v) <= budget {
            break;
        }
        let mut all = Vec::new();
        lists(&v, "", &mut all);
        // The list taking the most room, of those that can still be cut.
        let Some((path, n, _)) = all.into_iter().filter(|(_, n, _)| *n > 1).max_by_key(|(_, _, bytes)| *bytes) else { break };
        let keep = (n / 2).max(1);
        if let Some(Value::Array(list)) = at(&mut v, &path) {
            list.truncate(keep);
        }
        match cut.iter_mut().find(|(p, _, _)| *p == path) {
            Some(c) => c.2 = keep,
            None => cut.push((path, n, keep)),
        }
    }
    // Still over (a few huge texts): cut the longest strings.
    for _ in 0..50 {
        if size(&v) <= budget {
            break;
        }
        if !shorten_longest_string(&mut v) {
            break;
        }
    }
    let lists: Vec<Value> = cut.iter().map(|(p, total, shown)| json!({"path": p, "total": total, "shown": shown})).collect();
    if let Some(o) = v.as_object_mut() {
        o.insert(
            "truncated".into(),
            json!({
                "reply_kb": kb,
                "full_kb": (before as f64 / 1024.0).round(),
                "lists": lists,
                "how": "Ask for less: page_path, page_offset and page_limit page through one list; select (execute, batch steps) keeps only the paths you need; reply_kb raises the budget",
            }),
        );
    }
    v
}

fn shorten_longest_string(v: &mut Value) -> bool {
    fn longest<'a>(v: &'a mut Value, best: &mut Option<&'a mut String>) {
        match v {
            Value::String(s) => {
                if best.as_ref().is_none_or(|b| s.len() > b.len()) {
                    *best = Some(s);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| longest(x, best)),
            Value::Object(o) => o.values_mut().for_each(|x| longest(x, best)),
            _ => {}
        }
    }
    let mut best = None;
    longest(v, &mut best);
    match best {
        Some(s) if s.len() > 2048 => {
            let keep = s.char_indices().nth(s.chars().count() / 2).map_or(s.len(), |(i, _)| i);
            s.truncate(keep);
            s.push_str(" …(cut)");
            true
        }
        _ => false,
    }
}

/// Whether a call is being shaped (outside one, replies go as they are: tests, internal calls).
pub(crate) fn active() -> bool {
    ACTIVE.with(Cell::get)
}
