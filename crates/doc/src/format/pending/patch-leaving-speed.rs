//! A Patch feature on a loop of edges (`surface.patch` with `edges`) saves `speed`: how fast the
//! patch leaves its G1/G2 edges, as a multiple of the loop's own speed (#195). New patches save 2,
//! which rises over a round rim like a hemisphere and stays fair to the rim; a patch saved
//! before has none and keeps the speed 1 it was filled with, so an older design opens with the
//! same shapes. The last release refuses a design that has it (it refuses the newer format
//! number).
