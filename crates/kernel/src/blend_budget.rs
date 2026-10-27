//! A work budget for blends (fillets and chamfers, with the corner, vertex and loop blends they
//! build), as booleans have their attempt budget (`ops::with_attempt_budget`). A blend tries one
//! construction after another, a corner rounded first takes a whole blend of the edges left, and
//! a size search (`fillet_max`) repeats all of it, so on a large body whose edges no construction
//! takes, a refusal took most of an hour (#243). Under a budget each step a blend takes (a
//! construction tried, a corner rounded, a cutter cut, a result checked) costs one work unit, the
//! same on every machine; a step takes about the same time on bodies of any size (most of it is
//! the body's healing and meshing). When the budget runs out, every further step is refused at
//! once, and the blend is refused naming the step it ran out in and the edges it was given.

use std::cell::{Cell, RefCell};

use solidcraft_geom::Vec3;

use crate::{KernelError, Result};

/// The work units a blend feature gets: about twice the steps the slowest blend the kernel cases
/// and the ladder accept takes (27).
pub const BLEND_WORK: u64 = 64;
/// The work units a fillet that may reduce its radius gets ([`crate::fillet_max_each`]): each
/// radius it tries is a fillet of its own, and a 14 × 6 × 10 box's four edges reduced from 3.5 to 3
/// take 166 steps.
pub const BLEND_SEARCH_WORK: u64 = 6 * BLEND_WORK;

thread_local! {
    /// The work units left to the blends running under [`with_blend_budget`], if any.
    static LEFT: Cell<Option<u64>> = const { Cell::new(None) };
    /// The step the budget ran out in, once it has.
    static OUT_IN: RefCell<Option<String>> = const { RefCell::new(None) };
    /// How many blends deep the current step is (a blend inside a blend, such as the fillet of
    /// one edge at a rounded corner).
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// `f`, with the blends it runs on this thread allowed `units` work units between them (see the
/// module comment), and each body they heal healed once. Where they run out, the result is a
/// refusal naming the step and `edges` (the points that picked them), whatever `f` returned; a
/// budget inside another takes the smaller and uses up the outer one's too.
pub fn with_blend_budget<T, E: From<KernelError>>(
    units: u64,
    edges: &[Vec3],
    f: impl FnOnce() -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    struct Restore(Option<u64>, u64, Option<String>);
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
    let r = crate::boolean_record::with_heal_memo(f);
    match OUT_IN.with(|c| c.borrow().clone()) {
        Some(stage) => Err(E::from(ran_out(&stage, mine, edges))),
        None => r,
    }
}

/// The work units left under the innermost [`with_blend_budget`] on this thread, if any.
pub fn blend_work_left() -> Option<u64> {
    LEFT.with(|c| c.get())
}

/// Takes the work of one step of a blend, `stage`: one unit. Refused (the step isn't taken)
/// where the budget has run out.
pub(crate) fn spend(stage: &str) -> Result<()> {
    if OUT_IN.with(|c| c.borrow().is_some()) {
        return Err(exhausted());
    }
    LEFT.with(|c| match c.get() {
        None => Ok(()),
        Some(n) if n > 0 => {
            c.set(Some(n - 1));
            Ok(())
        }
        Some(_) => {
            OUT_IN.with(|o| *o.borrow_mut() = Some(stage.to_string()));
            Err(exhausted())
        }
    })
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

fn exhausted() -> KernelError {
    KernelError::Failed("the blend's work budget has run out".into())
}

fn ran_out(stage: &str, units: u64, edges: &[Vec3]) -> KernelError {
    let shown: Vec<String> = edges.iter().take(6).map(|p| format!("[{:.2}, {:.2}, {:.2}]", p.x, p.y, p.z)).collect();
    let more = if edges.len() > 6 { format!(" and {} more", edges.len() - 6) } else { String::new() };
    KernelError::Failed(format!(
        "not supported yet: the blend ran out of its work budget ({units} steps) while {stage}, on the edges at {}{more}: no construction takes these edges here (try fewer edges or another size)",
        shown.join(", ")
    ))
}
