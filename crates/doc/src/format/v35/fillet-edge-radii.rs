//! A constant-radius fillet can give each of its edges a radius of its own (`edge_radii`: one
//! expression per entry of `edges`, in order), the edges rounded together so the corners where
//! rounds of different radii meet are closed by one patch. Older designs need no transformation:
//! a fillet without it rounds every edge with `radius`, as before. The last release refuses a
//! design saved with this (the newer format number), so it can't drop the radii and round every
//! edge alike.
