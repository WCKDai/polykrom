//! A deterministic work budget for kernel operations that try one construction after another
//! (#243). A blend tries one construction after another, a corner rounded first takes a whole
//! blend of the edges left, and a size search (`fillet_max`) repeats all of it, so on a large
//! body whose edges no construction takes, a refusal took most of an hour; booleans retry with
//! shifted copies and other tolerances in the same way.
//!
//! Under [`with_work_budget`], each step an operation takes costs work units, counted on this
//! thread, the same on every machine. Steps are named and belong to a [`Stage`] (a blend, a
//! boolean, a tessellation…). When the units run out, every further step is refused at once, and
//! the operation is refused naming the stage and the step it ran out in and what it was working
//! on. Without a budget (the kernel's own API, its tests) nothing is counted.

use std::cell::{Cell, RefCell};

use solidcraft_geom::Vec3;

use crate::{KernelError, Result};

/// Which part of the kernel a step of work belongs to (named in a refusal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// A fillet or chamfer, with the corner, vertex and loop blends it builds.
    Blend,
    /// A boolean and its retries.
    Boolean,
    /// A tessellation.
    Tessellate,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Stage::Blend => "blend",
            Stage::Boolean => "boolean",
            Stage::Tessellate => "tessellation",
        }
    }
}

/// The work units a blend feature gets: one per step, about twice the steps the slowest blend
/// the kernel cases and the ladder accept takes (27).
pub const BLEND_WORK: u64 = 64;
/// The work units a fillet that may reduce its radius gets ([`crate::fillet_max_each`]): each
/// radius it tries is a fillet of its own, and a 14 × 6 × 10 box's four edges reduced from 3.5 to 3
/// take 166 steps.
pub const BLEND_SEARCH_WORK: u64 = 6 * BLEND_WORK;

thread_local! {
    /// The work units left to the operations running under [`with_work_budget`], if any.
    static LEFT: Cell<Option<u64>> = const { Cell::new(None) };
    /// The stage and step the budget ran out in, once it has.
    static OUT_IN: RefCell<Option<(Stage, String)>> = const { RefCell::new(None) };
    /// How many blends deep the current step is (a blend inside a blend, such as the fillet of
    /// one edge at a rounded corner).
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// `f`, with the work it runs on this thread allowed `units` work units (see the module
/// comment). Where they run out, the result is a refusal naming the stage, the step and
/// `subject` (what the operation works on, as a person reads it: "the edges at …"), whatever `f`
/// returned. A budget inside another takes the smaller and uses up the outer one's too.
pub fn with_work_budget<T, E: From<KernelError>>(
    units: u64,
    subject: &str,
    f: impl FnOnce() -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    struct Restore(Option<u64>, u64, Option<(Stage, String)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let left = LEFT.with(|c| c.get()).unwrap_or(0);
            LEFT.with(|c| c.set(self.0.map(|outer| outer.saturating_sub(self.1.saturating_sub(left)))));
            OUT_IN.with(|c| *c.borrow_mut() = self.2.take());
        }
    }
    let outer = LEFT.with(|c| c.get());
    let mine = outer.map_or(units, |o| o.min(units));
    let outer_out = OUT_IN.with(|c| c.borrow_mut().take());
    LEFT.with(|c| c.set(Some(mine)));
    let _restore = Restore(outer, mine, outer_out);
    let r = f();
    match OUT_IN.with(|c| c.borrow().clone()) {
        Some((stage, step)) => Err(E::from(ran_out(stage, &step, mine, subject))),
        None => r,
    }
}

/// [`with_work_budget`] for a blend of the edges picked at `edges`, each body it heals healed
/// once (a blend tries construction after construction on the same body).
pub fn with_blend_budget<T, E: From<KernelError>>(
    units: u64,
    edges: &[Vec3],
    f: impl FnOnce() -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    with_work_budget(units, &edges_subject(edges), || crate::boolean_record::with_heal_memo(f))
}

/// The work units left under the innermost [`with_work_budget`] on this thread, if any.
pub fn work_left() -> Option<u64> {
    LEFT.with(|c| c.get())
}

/// Takes `units` of work for the step `step` of `stage`. Refused (the step isn't taken) where the
/// budget has run out, or would by this step.
pub(crate) fn spend(stage: Stage, step: &str, units: u64) -> Result<()> {
    if OUT_IN.with(|c| c.borrow().is_some()) {
        return Err(exhausted(stage));
    }
    LEFT.with(|c| match c.get() {
        None => Ok(()),
        Some(n) if n >= units => {
            c.set(Some(n - units));
            Ok(())
        }
        Some(_) => {
            OUT_IN.with(|o| *o.borrow_mut() = Some((stage, step.to_string())));
            Err(exhausted(stage))
        }
    })
}

/// Whether a work budget has run out on this thread (an operation that can't count its own
/// steps, such as a boolean's retries, stops trying).
pub(crate) fn exhausted_now() -> bool {
    OUT_IN.with(|c| c.borrow().is_some())
}

/// One step of a blend: one unit.
pub(crate) fn blend_step(step: &str) -> Result<()> {
    spend(Stage::Blend, step, 1)
}

/// `f` as a blend: its own steps one level deeper than the blend that runs it, if any.
pub(crate) fn nested<T>(f: impl FnOnce() -> T) -> T {
    struct Up(usize);
    impl Drop for Up {
        fn drop(&mut self) {
            DEPTH.with(|d| d.set(self.0));
        }
    }
    let depth = DEPTH.with(|d| {
        let was = d.get();
        d.set(was + 1);
        was
    });
    let _up = Up(depth);
    f()
}

/// Whether the blend running is inside another (whose refusal, not this one's, is what a person
/// reads).
pub(crate) fn inside_another() -> bool {
    DEPTH.with(|d| d.get() > 1)
}

fn exhausted(stage: Stage) -> KernelError {
    KernelError::Failed(format!("the {}'s work budget has run out", stage.name()))
}

/// "the edges at [..], [..] and N more".
fn edges_subject(edges: &[Vec3]) -> String {
    let shown: Vec<String> = edges.iter().take(6).map(|p| format!("[{:.2}, {:.2}, {:.2}]", p.x, p.y, p.z)).collect();
    let more = if edges.len() > 6 { format!(" and {} more", edges.len() - 6) } else { String::new() };
    format!("the edges at {}{more}", shown.join(", "))
}

fn ran_out(stage: Stage, step: &str, units: u64, subject: &str) -> KernelError {
    KernelError::Failed(format!(
        "not supported yet: the {} ran out of its work budget ({units} units) while {step}, on {subject}: no construction takes this here (try fewer edges or another size)",
        stage.name()
    ))
}
