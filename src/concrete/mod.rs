//! Reinforced concrete design to SNI 2847, which adopts ACI 318.
//!
//! # Unit convention
//!
//! Strict SI: lengths in mm, areas in mm2, stresses in MPa, moments in N·mm.
//! See [`crate::units`].
//!
//! # Contents
//!
//! - [`flexure`]: singly reinforced rectangular beams, stress block, strain-based
//!   `phi`, and the minimum and maximum reinforcement limits
//!
//! # Design responsibility
//!
//! Everything in this module is a calculation aid. It does not replace a licensed
//! engineer's judgement, and every result must be checked against the governing
//! edition of SNI 2847 before use in a real design.

pub mod flexure;

pub use flexure::{
    beta1, check_beam_flexure, maximum_reinforcement, minimum_reinforcement,
    strength_reduction_factor, ConcreteBeamInput, ConcreteFlexureCheck, Confinement,
    SectionBehaviour, REBAR_ES,
};

/// The limit state that governs a concrete flexural check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConcreteLimitState {
    /// Flexural strength, `Mu <= phi*Mn` (SNI 2847 clause 9.5.1.1).
    FlexuralStrength,
    /// Minimum tension reinforcement (clause 9.6.1.2). The section is too lightly
    /// reinforced and could fail on first cracking.
    MinimumReinforcement,
    /// Maximum tension reinforcement, that is the `eps_t >= 0.004` ductility limit
    /// (clause 9.3.3.1). The section is too heavily reinforced to fail ductilely.
    MaximumReinforcement,
}

impl core::fmt::Display for ConcreteLimitState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text = match self {
            Self::FlexuralStrength => "flexural strength",
            Self::MinimumReinforcement => "minimum reinforcement",
            Self::MaximumReinforcement => "maximum reinforcement (ductility)",
        };
        f.write_str(text)
    }
}
