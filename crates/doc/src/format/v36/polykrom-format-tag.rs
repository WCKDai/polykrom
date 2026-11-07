//! A design's `format` tag is `polykrom/N` (#304, the second rename stage); designs saved before
//! say `solidcraft/N`, or `solvecraft/N` before that, and open as before (no transformation:
//! only the tag changes, and the next save writes the new one). The last release refuses a
//! design saved with this (the newer format number, and a tag it doesn't know), and so does
//! SolidCraft: a Polykrom design no longer opens in SolidCraft.
