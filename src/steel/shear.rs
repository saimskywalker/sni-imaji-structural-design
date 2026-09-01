//! Shear strength of I-shaped members, SNI 1729 chapter G.
//!
//! Units: mm, mm2, MPa, N. See [`crate::units`].
//!
//! The nominal strength is `Vn = 0.6 * Fy * Aw * Cv`, with `Aw = d * tw` for a
//! rolled I shape (clause G2.1 uses the overall depth, not the clear web depth).
//! The web shear coefficient `Cv` and the resistance factor `phi_v` both depend on
//! the web slenderness `h/tw`.
//!
//! # Scope
//!
//! Unstiffened and transversely stiffened webs of doubly symmetric I shapes, by
//! clause G2.1 alone. The tension field action of clause G2.2 is not implemented,
//! so the result is conservative for a stiffened girder that could mobilise it.

use crate::error::{positive, Result};
use crate::steel::{LimitState, SteelSection, STEEL_E};
use crate::units::{Area, Force, Stress};

/// Web plate buckling coefficient for a web without transverse stiffeners.
///
/// SNI 1729 clause G2.1(b)(i). For a stiffened web use
/// [`stiffened_web_kv`] instead.
pub const KV_UNSTIFFENED: f64 = 5.34;

/// Which branch of clause G2.1 produced `Cv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WebShearCase {
    /// `h/tw <= 2.24*sqrt(E/Fy)`. The web yields in shear before it buckles, and
    /// clause G2.1(a) allows `phi_v = 1.00` with `Cv = 1.0`.
    StockyWebFullYield,
    /// `h/tw <= 1.10*sqrt(kv*E/Fy)`. Shear yielding still governs, but with the
    /// general `phi_v = 0.90`.
    ShearYielding,
    /// `1.10*sqrt(kv*E/Fy) < h/tw <= 1.37*sqrt(kv*E/Fy)`. Inelastic web buckling,
    /// `Cv` falls off as `1/(h/tw)`.
    InelasticWebBuckling,
    /// `h/tw > 1.37*sqrt(kv*E/Fy)`. Elastic web buckling, `Cv` falls off as
    /// `1/(h/tw)^2`.
    ElasticWebBuckling,
}

/// Input to a shear check.
///
/// Build with [`ShearInput::new`], which defaults `E` to 200 000 MPa and `kv` to
/// [`KV_UNSTIFFENED`].
#[derive(Debug, Clone, PartialEq)]
pub struct ShearInput {
    /// The section being checked.
    pub section: SteelSection,
    /// Specified minimum yield stress `Fy`, MPa.
    pub fy: Stress,
    /// Modulus of elasticity `E`, MPa.
    pub e: Stress,
    /// Web plate buckling coefficient `kv`, dimensionless.
    pub kv: f64,
    /// Required shear strength `Vu`, N.
    pub vu: Force,
}

impl ShearInput {
    /// A check with `E = 200 000 MPa` and an unstiffened web (`kv = 5.34`).
    ///
    /// Units: `fy` in MPa, `vu` in N.
    pub fn new(section: SteelSection, fy: Stress, vu: Force) -> Self {
        Self {
            section,
            fy,
            e: STEEL_E,
            kv: KV_UNSTIFFENED,
            vu,
        }
    }

    /// Override `kv`, for example with [`stiffened_web_kv`].
    #[must_use]
    pub fn with_kv(mut self, kv: f64) -> Self {
        self.kv = kv;
        self
    }

    /// Override the modulus of elasticity, in MPa.
    #[must_use]
    pub fn with_elastic_modulus(mut self, e: Stress) -> Self {
        self.e = e;
        self
    }
}

/// The full result of a shear check.
#[derive(Debug, Clone, PartialEq)]
pub struct ShearCheck {
    /// Web slenderness `h/tw`, dimensionless.
    pub h_over_tw: f64,
    /// The stocky-web threshold `2.24*sqrt(E/Fy)`.
    pub stocky_limit: f64,
    /// The shear yielding threshold `1.10*sqrt(kv*E/Fy)`.
    pub yield_limit: f64,
    /// The inelastic buckling threshold `1.37*sqrt(kv*E/Fy)`.
    pub inelastic_limit: f64,
    /// `kv` used.
    pub kv: f64,
    /// Web shear coefficient `Cv`, dimensionless, at most 1.0.
    pub cv: f64,
    /// Which branch of clause G2.1 applied.
    pub case: WebShearCase,
    /// Web area `Aw = d * tw`, mm2.
    pub aw: Area,
    /// Nominal shear strength `Vn = 0.6*Fy*Aw*Cv`, N.
    pub vn: Force,
    /// Resistance factor used, 1.00 for the stocky-web case and 0.90 otherwise.
    pub phi_v: f64,
    /// Design shear strength `phi_v * Vn`, N.
    pub phi_vn: Force,
    /// Required shear strength `Vu`, N.
    pub vu: Force,
    /// Demand over capacity, `Vu / (phi_v * Vn)`.
    pub ratio: f64,
    /// The governing limit state, always [`LimitState::Shear`] here.
    pub governing: LimitState,
    /// True when `ratio <= 1.0`.
    pub adequate: bool,
}

/// Web plate buckling coefficient for a web with transverse stiffeners.
///
/// `kv = 5 + 5/(a/h)^2`, where `a` is the clear stiffener spacing and `h` the
/// clear web depth (SNI 1729 clause G2.1(b)). Returns [`KV_UNSTIFFENED`] when the
/// stiffeners are so far apart that the stiffened value falls below it.
///
/// # Errors
///
/// Errors if `a_over_h` is not a positive finite number.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 1729.
pub fn stiffened_web_kv(a_over_h: f64) -> Result<f64> {
    let ratio = positive("a_over_h", a_over_h)?;
    Ok((5.0 + 5.0 / (ratio * ratio)).max(KV_UNSTIFFENED))
}

/// Full shear check of an I-shaped member, SNI 1729 clause G2.1.
///
/// Units: strict SI, mm / MPa / N.
///
/// # Errors
///
/// Errors if the section geometry is invalid, or if `fy`, `e` or `kv` are not
/// positive and finite, or if `vu` is negative or not finite.
///
/// # Design responsibility
///
/// This is a calculation aid, not a substitute for a licensed engineer's
/// judgement. Check every result against the governing edition of SNI 1729.
///
/// # Examples
///
/// ```
/// use sni_struct::steel::{check_shear, profiles, ShearInput, WebShearCase};
/// use sni_struct::units::{Force, Stress};
///
/// let section = profiles::wf("WF 400x200x8x13").expect("profile exists");
/// let input = ShearInput::new(section, Stress::mpa(240.0), Force::kn(200.0));
/// let check = check_shear(&input)?;
///
/// // h/tw = 342/8 = 42.75 <= 2.24*sqrt(200000/240) = 64.66, so Cv = 1.0 and phi_v = 1.00.
/// assert_eq!(check.case, WebShearCase::StockyWebFullYield);
/// // Vn = 0.6 * 240 * (400*8) * 1.0 = 460 800 N
/// assert!((check.vn.to_kn() - 460.8).abs() < 1e-6);
/// assert!(check.adequate);
/// # Ok::<(), sni_struct::SniError>(())
/// ```
pub fn check_shear(input: &ShearInput) -> Result<ShearCheck> {
    let section = &input.section;
    section.validate()?;
    let fy = positive("fy", input.fy.to_mpa())?;
    let e = positive("e", input.e.to_mpa())?;
    let kv = positive("kv", input.kv)?;
    let vu = crate::error::non_negative("vu", input.vu.to_n())?;

    let h_over_tw = section.web_slenderness();
    let stocky_limit = 2.24 * (e / fy).sqrt();
    let yield_limit = 1.10 * (kv * e / fy).sqrt();
    let inelastic_limit = 1.37 * (kv * e / fy).sqrt();

    let (cv, phi_v, case) = if h_over_tw <= stocky_limit {
        (1.0, 1.00, WebShearCase::StockyWebFullYield)
    } else if h_over_tw <= yield_limit {
        (1.0, 0.90, WebShearCase::ShearYielding)
    } else if h_over_tw <= inelastic_limit {
        (
            yield_limit / h_over_tw,
            0.90,
            WebShearCase::InelasticWebBuckling,
        )
    } else {
        (
            1.51 * e * kv / (h_over_tw * h_over_tw * fy),
            0.90,
            WebShearCase::ElasticWebBuckling,
        )
    };
    let cv = cv.min(1.0);

    let aw = section.web_area();
    let vn = Force::n(0.6 * fy * aw.to_mm2() * cv);
    let phi_vn = vn * phi_v;
    let ratio = if phi_vn.to_n() > 0.0 {
        vu / phi_vn.to_n()
    } else {
        f64::INFINITY
    };

    Ok(ShearCheck {
        h_over_tw,
        stocky_limit,
        yield_limit,
        inelastic_limit,
        kv,
        cv,
        case,
        aw,
        vn,
        phi_v,
        phi_vn,
        vu: input.vu,
        ratio,
        governing: LimitState::Shear,
        adequate: ratio <= 1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SniError;
    use crate::steel::profiles;
    use crate::steel::section::SteelSection;
    use crate::units::{Area, Length, SecondMoment, SectionModulus};

    const BJ37: Stress = Stress::mpa(240.0);

    #[test]
    fn stocky_web_gets_full_yield_and_unit_phi() {
        let section = profiles::wf("WF 400x200x8x13").expect("profile");
        let check = check_shear(&ShearInput::new(section, BJ37, Force::kn(200.0))).expect("check");
        // h/tw = 342/8 = 42.75, limit = 2.24*28.86751 = 64.663
        assert!((check.h_over_tw - 42.75).abs() < 1e-9);
        assert!((check.stocky_limit - 64.663).abs() < 0.01);
        assert_eq!(check.case, WebShearCase::StockyWebFullYield);
        assert!((check.phi_v - 1.00).abs() < 1e-12);
        // Vn = 0.6*240*3200 = 460800 N
        assert!((check.vn.to_n() - 460_800.0).abs() < 1e-6);
        assert!((check.phi_vn.to_n() - 460_800.0).abs() < 1e-6);
        // ratio = 200000/460800 = 0.43403
        assert!((check.ratio - 0.434_03).abs() < 1e-5);
    }

    /// A thin web that lands past the elastic buckling threshold.
    fn thin_web_girder(tw_mm: f64) -> SteelSection {
        let depth = Length::mm(1000.0);
        let bf = Length::mm(300.0);
        let tw = Length::mm(tw_mm);
        let tf = Length::mm(20.0);
        SteelSection {
            designation: format!("PG 1000x300x{tw_mm}x20"),
            depth,
            flange_width: bf,
            web_thickness: tw,
            flange_thickness: tf,
            fillet_radius: Length::mm(0.0),
            area: Area::mm2(2.0 * 300.0 * 20.0 + 960.0 * tw_mm),
            ix: SecondMoment::mm4(2.0e9),
            iy: SecondMoment::mm4(9.0e7),
            sx: SectionModulus::mm3(4.0e6),
            zx: SteelSection::plastic_modulus_from_geometry(depth, bf, tw, tf),
        }
    }

    #[test]
    fn elastic_web_buckling_branch() {
        // tw = 4 mm: h/tw = 960/4 = 240
        // 1.37*sqrt(5.34*200000/240) = 1.37*66.71 = 91.4, so 240 > 91.4
        // Cv = 1.51*200000*5.34/(240^2*240) = 1612680/13824000 = 0.11666
        let check = check_shear(&ShearInput::new(
            thin_web_girder(4.0),
            BJ37,
            Force::kn(50.0),
        ))
        .expect("check");
        assert_eq!(check.case, WebShearCase::ElasticWebBuckling);
        assert!((check.cv - 0.116_66).abs() < 1e-4, "cv {}", check.cv);
        assert!((check.phi_v - 0.90).abs() < 1e-12);
    }

    #[test]
    fn inelastic_web_buckling_branch() {
        // Need 1.10*sqrt(kv*E/Fy) < h/tw <= 1.37*sqrt(...), i.e. 73.4 < h/tw <= 91.4
        // tw = 12 mm gives h/tw = 960/12 = 80.
        let check = check_shear(&ShearInput::new(
            thin_web_girder(12.0),
            BJ37,
            Force::kn(50.0),
        ))
        .expect("check");
        assert_eq!(check.case, WebShearCase::InelasticWebBuckling);
        // Cv = 1.10*sqrt(5.34*200000/240)/80 = 73.386/80 = 0.91733
        assert!((check.cv - 0.917_33).abs() < 1e-4, "cv {}", check.cv);
    }

    #[test]
    fn shear_yielding_branch_between_the_two_limits() {
        // Needs stocky_limit < h/tw <= yield_limit, i.e. 64.663 < h/tw <= 73.386.
        // tw = 14 mm gives h/tw = 960/14 = 68.57.
        let check = check_shear(&ShearInput::new(
            thin_web_girder(14.0),
            BJ37,
            Force::kn(50.0),
        ))
        .expect("check");
        assert_eq!(check.case, WebShearCase::ShearYielding);
        assert!((check.cv - 1.0).abs() < 1e-12);
        assert!((check.phi_v - 0.90).abs() < 1e-12);
    }

    #[test]
    fn stiffened_kv_never_drops_below_unstiffened() {
        // a/h = 1.0 -> kv = 10.0
        assert!((stiffened_web_kv(1.0).expect("kv") - 10.0).abs() < 1e-12);
        // a/h = 3.0 -> 5 + 5/9 = 5.556, above 5.34
        assert!((stiffened_web_kv(3.0).expect("kv") - 5.5556).abs() < 1e-4);
        // a/h = 10 -> 5.05, clamped up to 5.34
        assert!((stiffened_web_kv(10.0).expect("kv") - KV_UNSTIFFENED).abs() < 1e-12);
        assert!(matches!(
            stiffened_web_kv(0.0),
            Err(SniError::NonPositive { .. })
        ));
    }

    #[test]
    fn negative_demand_is_rejected() {
        let section = profiles::wf("WF 400x200x8x13").expect("profile");
        let err =
            check_shear(&ShearInput::new(section, BJ37, Force::kn(-5.0))).expect_err("must reject");
        assert!(matches!(
            err,
            SniError::Negative {
                parameter: "vu",
                ..
            }
        ));
    }

    #[test]
    fn overloaded_web_is_inadequate() {
        let section = profiles::wf("WF 400x200x8x13").expect("profile");
        let check = check_shear(&ShearInput::new(section, BJ37, Force::kn(600.0))).expect("check");
        assert!(!check.adequate);
        assert!(check.ratio > 1.0);
    }
}
