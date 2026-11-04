//! A material-changing feature that evaluates and changes nothing (polykrom#309): a hole drilled
//! into the air, a cut clear of every body, a combine whose tool misses its target. Such a
//! feature reports success, and an agent that trusts the reply goes on from a part it did not
//! change. [`no_effect`] judges it from the states around the feature's one evaluation, without
//! evaluating anything again: none of the bodies it targets was made, removed or given another
//! shape. "Another shape" is [`Body::same_shape`], a check of shared data: an operation that
//! leaves a body as it is hands the same body on, so the check costs a pointer comparison per
//! body and never a measurement.

use super::*;
use crate::BodyName;

/// A feature that changed none of the bodies it targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoEffect {
    /// The feature's name.
    pub feature: String,
    /// The bodies it targeted and left as they were, in the model's order.
    pub bodies: Vec<String>,
}

impl NoEffect {
    /// What the feature's warning says.
    pub fn message(&self) -> String {
        let names: Vec<String> = self.bodies.iter().map(|b| format!("`{b}`")).collect();
        let bodies = match names.as_slice() {
            [] => "nothing".to_string(),
            [one] => format!("{one} is"),
            [rest @ .., last] => format!("{} and {last} are", rest.join(", ")),
        };
        format!("{} has no effect: {bodies} unchanged", self.feature)
    }
}

/// The bodies a material-changing feature works on.
enum Targets {
    /// The feature's boolean scope: its targets, else its participants, else its component.
    Boolean(Vec<String>),
    /// The one body it names.
    Named(String),
    /// The body nearest the first picked edge or face (a blend, shell or draft naming none).
    Nearest { points: Vec<Vec3>, pick: Pick },
}

/// How a feature naming no body finds the one it works on.
#[derive(Clone, Copy)]
enum Pick {
    Edge,
    Face,
}

/// Which bodies `f` changes when it works (`None`: a feature that is not expected to change
/// material: a sketch, a new body, a move, a pattern…).
fn targets(f: &Feature) -> Option<Targets> {
    let one = |body: &Option<BodyName>, points: Vec<Vec3>, pick: Pick| match body {
        Some(name) => Targets::Named(name.to_string()),
        None => Targets::Nearest { points, pick },
    };
    Some(match &f.kind {
        FeatureKind::Extrude { operation, targets, .. }
        | FeatureKind::Revolve { operation, targets, .. }
        | FeatureKind::Loft { operation, targets, .. }
        | FeatureKind::Sweep { operation, targets, .. } => {
            if *operation == Operation::NewBody {
                return None;
            }
            Targets::Boolean(crate::plain(targets))
        }
        FeatureKind::Box { operation, .. }
        | FeatureKind::Cylinder { operation, .. }
        | FeatureKind::Sphere { operation, .. }
        | FeatureKind::Torus { operation, .. } => {
            if *operation == Operation::NewBody {
                return None;
            }
            Targets::Boolean(Vec::new())
        }
        FeatureKind::Hole { .. } => Targets::Boolean(Vec::new()),
        FeatureKind::Fillet { edges, body, .. } | FeatureKind::Chamfer { edges, body, .. } => one(body, crate::plain(edges), Pick::Edge),
        FeatureKind::Shell { faces, body, .. } | FeatureKind::Draft { faces, body, .. } => one(body, crate::plain(faces), Pick::Face),
        FeatureKind::Combine { target, .. } => Targets::Named(target.to_string()),
        FeatureKind::Split { body, .. } => Targets::Named(body.to_string()),
        _ => return None,
    })
}

/// The bodies of `f` (a material-changing feature) that it left exactly as they were, when it
/// changed none of them: `before` and `after` are the model around its evaluation. `None` when
/// the feature made, removed or reshaped a body it targets, or is not a material-changing one.
pub fn no_effect(f: &Feature, before: &ModelState, after: &ModelState) -> Option<NoEffect> {
    let targets = targets(f)?;
    // A body it made (a join clear of every target, the pieces of a cut) is a change.
    if after.bodies.iter().any(|b| b.feature == f.id || before.body(&b.name).is_none()) {
        return None;
    }
    let unchanged = |name: &str| match (before.body(name), after.body(name)) {
        (Some(was), Some(now)) => was.body.same_shape(&now.body),
        _ => false,
    };
    let bodies = match targets {
        Targets::Boolean(names) => boolean_scope(before, f, &names).1,
        Targets::Named(name) => vec![name],
        Targets::Nearest { points, pick } => {
            // Which body the picks are on is worked out only once every body is known to be as
            // it was: the rare case, so the search costs nothing when the feature works.
            if before.bodies.len() != after.bodies.len() || !before.bodies.iter().all(|b| unchanged(&b.name)) {
                return None;
            }
            let i = match pick {
                Pick::Edge => blend_target(before, &None, &points).ok()?,
                Pick::Face => body_at(before, &None, &points).ok()?,
            };
            vec![before.bodies.get(i)?.name.clone()]
        }
    };
    if bodies.is_empty() || !bodies.iter().all(|b| unchanged(b)) {
        return None;
    }
    Some(NoEffect { feature: f.name.clone(), bodies })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn feature(kind: FeatureKind) -> Feature {
        let mut doc = Document::default();
        let id = doc.add_feature(kind, Some("Cut1")).unwrap();
        doc.feature(id).unwrap().clone()
    }

    fn state(bodies: Vec<ModelBody>) -> ModelState {
        ModelState { bodies, ..Default::default() }
    }

    fn block(name: &str) -> ModelBody {
        // Made by an earlier feature than the one judged (which `feature` numbers 1).
        ModelBody::new(name.to_string(), kernel::box_solid(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)).unwrap(), 7)
    }

    fn boxed(operation: Operation) -> Feature {
        feature(FeatureKind::Box {
            corner: ["0".into(), "0".into(), "20".into()],
            length: "5".into(),
            width: "5".into(),
            height: "5".into(),
            operation,
        })
    }

    fn cut() -> Feature {
        boxed(Operation::Cut)
    }

    #[test]
    fn the_same_bodies_handed_on_are_no_effect() {
        let a = block("A");
        let b = block("B");
        let before = state(vec![a.clone(), b.clone()]);
        let after = state(vec![a, b]);
        let n = no_effect(&cut(), &before, &after).unwrap();
        assert_eq!(n.bodies, ["A", "B"]);
        assert_eq!(n.message(), "Cut1 has no effect: `A` and `B` are unchanged");
        let one = NoEffect { feature: "Hole1".into(), bodies: vec!["Plate".into()] };
        assert_eq!(one.message(), "Hole1 has no effect: `Plate` is unchanged");
    }

    #[test]
    fn a_body_with_another_shape_a_new_body_or_a_lost_body_is_a_change() {
        let a = block("A");
        let before = state(vec![a.clone()]);
        // The same geometry, built again: another shape as far as the check goes (it never measures).
        assert!(no_effect(&cut(), &before, &state(vec![block("A")])).is_none());
        assert!(no_effect(&cut(), &before, &state(vec![a.clone(), block("A (1)")])).is_none());
        assert!(no_effect(&cut(), &before, &state(vec![])).is_none());
        // A feature that makes bodies is never judged, however it leaves the others.
        assert!(no_effect(&boxed(Operation::NewBody), &before, &state(vec![a])).is_none());
    }
}
