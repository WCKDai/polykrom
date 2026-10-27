//! A feature can keep `own_chain_depth` (true): the bodies it makes or changes are named from
//! history while their own chain of features (the bodies their faces come from) is shorter than
//! the naming limit, not while the whole design is, so a cut late in a long design keeps its
//! target's face names and the joints and features that use them (#143). Older designs need no
//! transformation: a feature without it counts every body in the design, so the faces of the
//! bodies it makes keep the names they were saved with (by position past the limit). The last
//! release refuses a design saved with this (the newer format number), so it can't drop the flag
//! and rename those faces.
