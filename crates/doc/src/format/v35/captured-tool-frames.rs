//! A join, cut or intersect with drawn tools can keep where each body it changes in another
//! component was placed when it was made (`tool_frames`: each `[body name, matrix]`, the matrix
//! from the feature's frame into the body's, #168), so driving a joint never reshapes the body.
//! Older designs need no transformation: a feature without frames meets its targets where they
//! are placed, as before, until it is edited. The last release refuses a design saved with this
//! (the newer format number), so it can't drop the frames and reshape the parts.
