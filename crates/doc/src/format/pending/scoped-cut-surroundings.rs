//! An Extrude or Revolve can keep `scoped_surroundings` (true): as a join or cut naming no target,
//! it shapes its tool around the bodies it can change (its participants, else its component's
//! bodies) instead of every body in the model (#219), so an unrelated part never reshapes it.
//! Older designs need no transformation: a feature without it shapes its tool around every body,
//! as before, and evaluates to the same geometry. The last release refuses a design saved with
//! this (the newer format number), so it can't drop the flag and reshape the cuts.
