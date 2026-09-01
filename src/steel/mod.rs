//! Steel member design to SNI 1729, which adopts AISC 360 in LRFD form.
//!
//! # Unit convention
//!
//! Strict SI throughout: lengths in mm, areas in mm2, section moduli in mm3,
//! second moments in mm4, stresses and moduli in MPa, forces in N, moments in
//! N·mm. Every public signature takes and returns the newtypes in
//! [`crate::units`], so a kN cannot be mistaken for a N.
//!
//! # Contents
//!
//! - [`section`]: geometry, torsional properties and slenderness classification
//! - [`profiles`]: a small catalogue of common Indonesian WF and H profiles
//! - [`flexure`]: chapter F, yielding, lateral torsional buckling, flange local buckling
//! - [`shear`]: chapter G, web shear
//!
//! # Design responsibility
//!
//! Everything in this module is a calculation aid. It does not replace a licensed
//! engineer's judgement, and every result must be checked against the governing
//! edition of SNI 1729 before use in a real design.

pub mod flexure;
pub mod profiles;
pub mod section;
pub mod shear;

pub use flexure::{
    check_flexure, flange_local_buckling_strength, lateral_torsional_buckling_strength, lp, lr,
    FlexureCheck, FlexureInput, LtbZone, PHI_B,
};
pub use section::{classify_flexural_section, SectionClassification, Slenderness, SteelSection};
pub use shear::{
    check_shear, stiffened_web_kv, ShearCheck, ShearInput, WebShearCase, KV_UNSTIFFENED,
};

use crate::units::Stress;

/// Modulus of elasticity of structural steel, `E = 200 000 MPa`.
///
/// SNI 1729 uses this value throughout chapters E, F and G.
pub const STEEL_E: Stress = Stress::mpa(200_000.0);

/// The limit state that produced a nominal strength.
///
/// Reported by every steel check so the engineer can see which clause governed
/// without re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitState {
    /// Full plastic moment reached, `Mn = Mp = Zx*Fy` (SNI 1729 clause F2.1).
    Yielding,
    /// Lateral torsional buckling of the compression flange (clause F2.2).
    LateralTorsionalBuckling,
    /// Local buckling of the compression flange (clause F3.2).
    FlangeLocalBuckling,
    /// Web shear yielding or buckling (clause G2.1).
    Shear,
}

impl core::fmt::Display for LimitState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text = match self {
            Self::Yielding => "yielding (Mp)",
            Self::LateralTorsionalBuckling => "lateral torsional buckling",
            Self::FlangeLocalBuckling => "flange local buckling",
            Self::Shear => "web shear",
        };
        f.write_str(text)
    }
}
