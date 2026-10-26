//! A Blend Surface feature (`surface.blend`) saves `tangent`: how fast each end of the blend
//! leaves its boundary, as a share of the chord across (#195). New blends save 1, the Hermite
//! blend's natural speed; a blend saved before has none and keeps the third it was built with,
//! so an older design opens with the same shapes. The last release refuses a design that has
//! it (it refuses the newer format number).
