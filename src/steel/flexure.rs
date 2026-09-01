//! Flexural strength of doubly symmetric I sections, SNI 1729 chapter F.
//!
//! SNI 1729 adopts AISC 360 in LRFD form. The clauses implemented here are F2
//! (compact I sections bent about the strong axis, yielding and lateral torsional
//! buckling) and F3 (the same with a non-compact or slender compression flange,
//! flange local buckling).
//!
//! Units: mm, mm2, mm3, mm4, MPa, N·mm. See [`crate::units`].
//!
//! # Scope and limitations
//!
//! - Doubly symmetric rolled I shapes bending about the strong axis only.
//! - The web must be compact. Non-compact and slender webs need clauses F4 and F5,
//!   which are not implemented; such a section returns
//!   [`SniError::UnsupportedSection`] rather than a wrong number.
//! - Tension flange yielding (F2 does not govern it for doubly symmetric shapes)
//!   and web local yielding under concentrated loads are out of scope.
//! - `Cb` is an input. Compute it from the moment diagram of the segment being
//!   checked; the default of 1.0 is always safe.

use crate::error::{in_range, non_negative, positive, Result, SniError};
use crate::steel::section::{classify_flexural_section, SectionClassification, Slenderness};
use crate::steel::{LimitState, SteelSection, STEEL_E};
use crate::units::{Length, Moment, Stress};

/// Resistance factor for flexure, `phi_b = 0.90` (SNI 1729 clause F1).
pub const PHI_B: f64 = 0.90;

/// Which of the three lateral torsional buckling zones the unbraced length falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LtbZone {
    /// `Lb <= Lp`. The full plastic moment is available; LTB does not reduce it.
    Plastic,
    /// `Lp < Lb <= Lr`. Inelastic LTB, linear interpolation of clause F2-2.
    Inelastic,
    /// `Lb > Lr`. Elastic LTB, the critical stress of clause F2-3 and F2-4.
    Elastic,
}

/// Everything needed for a strong-axis flexural check.
///
/// Build with [`FlexureInput::new`], which defaults `E` to 200 000 MPa and `Cb`
/// to 1.0, then override with the `with_*` methods.
#[derive(Debug, Clone, PartialEq)]
pub struct FlexureInput {
    /// The section being checked.
    pub section: SteelSection,
    /// Specified minimum yield stress `Fy`, MPa.
    pub fy: Stress,
    /// Modulus of elasticity `E`, MPa. Normally 200 000.
    pub e: Stress,
    /// Unbraced length `Lb` of the compression flange, mm. Zero means continuously braced.
    pub lb: Length,
    /// Lateral torsional buckling modification factor `Cb`, dimensionless.
    ///
    /// SNI 1729 clause F1. 1.0 is always conservative. Must lie in 1.0 to 3.0.
    pub cb: f64,
    /// Required flexural strength `Mu` from the governing load combination, N·mm.
    pub mu: Moment,
}

impl FlexureInput {
    /// A check with `E = 200 000 MPa` and `Cb = 1.0`.
    ///
    /// Units: `fy` in MPa, `lb` in mm, `mu` in N·mm. Use the named constructors on
    /// [`Stress`], [`Length`] and [`Moment`] so the unit is visible at the call site.
    pub fn new(section: SteelSection, fy: Stress, lb: Length, mu: Moment) -> Self {
        Self {
            section,
            fy,
            e: STEEL_E,
            lb,
            cb: 1.0,
            mu,
        }
    }

    /// Override the lateral torsional buckling modification factor `Cb`.
    #[must_use]
    pub fn with_cb(mut self, cb: f64) -> Self {
        self.cb = cb;
        self
    }

    /// Override the modulus of elasticity, in MPa.
    #[must_use]
    pub fn with_elastic_modulus(mut self, e: Stress) -> Self {
        self.e = e;
        self
    }
}

/// The full result of a flexural check, with every intermediate value an engineer
/// would want in order to reproduce it by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct FlexureCheck {
    /// Slenderness classification of flange and web.
    pub classification: SectionClassification,
    /// Plastic moment `Mp = Zx * Fy`, N·mm.
    pub mp: Moment,
    /// Yield moment `My = Sx * Fy`, N·mm. Reported for reference.
    pub my: Moment,
    /// Limiting unbraced length for the plastic moment, `Lp`, mm.
    pub lp: Length,
    /// Limiting unbraced length for inelastic LTB, `Lr`, mm.
    pub lr: Length,
    /// Unbraced length used, mm.
    pub lb: Length,
    /// Which LTB zone `Lb` falls in.
    pub ltb_zone: LtbZone,
    /// `Cb` used.
    pub cb: f64,
    /// Effective radius of gyration `rts`, mm.
    pub rts: Length,
    /// Radius of gyration about the weak axis `ry`, mm.
    pub ry: Length,
    /// Torsional constant `J`, mm4.
    pub j: f64,
    /// Distance between flange centroids `ho`, mm.
    pub ho: Length,
    /// Nominal strength governed by lateral torsional buckling, N·mm.
    pub mn_ltb: Moment,
    /// Nominal strength governed by flange local buckling, N·mm.
    ///
    /// `None` when the flange is compact, in which case FLB cannot govern.
    pub mn_flb: Option<Moment>,
    /// Governing nominal flexural strength `Mn`, N·mm.
    pub mn: Moment,
    /// Resistance factor used, 0.90.
    pub phi_b: f64,
    /// Design flexural strength `phi_b * Mn`, N·mm.
    pub phi_mn: Moment,
    /// Required flexural strength `Mu`, N·mm.
    pub mu: Moment,
    /// Demand over capacity, `Mu / (phi_b * Mn)`. At or below 1.0 the section passes.
    pub ratio: f64,
    /// Which limit state produced `Mn`.
    pub governing: LimitState,
    /// True when `ratio <= 1.0`.
    pub adequate: bool,
}

/// Limiting unbraced length for the plastic moment, `Lp = 1.76 * ry * sqrt(E/Fy)`.
///
/// SNI 1729 equation F2-5. Units: `fy`, `e` in MPa; result in mm.
///
/// # Errors
///
/// Errors if the section is invalid or `fy`, `e` are not positive and finite.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 1729.
pub fn lp(section: &SteelSection, fy: Stress, e: Stress) -> Result<Length> {
    section.validate()?;
    let fy = positive("fy", fy.to_mpa())?;
    let e = positive("e", e.to_mpa())?;
    let ry = section.ry()?.to_mm();
    Ok(Length::mm(1.76 * ry * (e / fy).sqrt()))
}

/// Limiting unbraced length for inelastic lateral torsional buckling, `Lr`.
///
/// SNI 1729 equation F2-6, with `c = 1.0` for a doubly symmetric I shape:
///
/// ```text
/// Lr = 1.95 * rts * E/(0.7*Fy) * sqrt( X + sqrt( X^2 + 6.76*(0.7*Fy/E)^2 ) )
/// X  = J*c / (Sx*ho)
/// ```
///
/// `J` comes from the thin-walled approximation in
/// [`SteelSection::torsional_constant`], which ignores root fillets and so gives a
/// slightly short, conservative `Lr`.
///
/// Units: `fy`, `e` in MPa; result in mm.
///
/// # Errors
///
/// Errors if the section is invalid or `fy`, `e` are not positive and finite.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 1729.
pub fn lr(section: &SteelSection, fy: Stress, e: Stress) -> Result<Length> {
    section.validate()?;
    let fy = positive("fy", fy.to_mpa())?;
    let e = positive("e", e.to_mpa())?;
    let rts = section.rts()?.to_mm();
    let sx = positive("sx", section.sx.to_mm3())?;
    let ho = section.flange_centroid_distance().to_mm();
    let j = section.torsional_constant();

    let x = j / (sx * ho);
    let fl = 0.7 * fy;
    let term = x + (x * x + 6.76 * (fl / e).powi(2)).sqrt();
    Ok(Length::mm(1.95 * rts * (e / fl) * term.sqrt()))
}

/// Nominal flexural strength limited by lateral torsional buckling, SNI 1729 F2.
///
/// Returns the strength and the zone it came from. Capped at `Mp` in every zone.
///
/// Units: `fy`, `e` in MPa; `lb` in mm; result in N·mm.
///
/// # Errors
///
/// Errors if the section is invalid, `fy` or `e` are not positive and finite,
/// `lb` is negative or not finite, or `cb` is outside 1.0 to 3.0.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 1729.
pub fn lateral_torsional_buckling_strength(
    section: &SteelSection,
    fy: Stress,
    e: Stress,
    lb: Length,
    cb: f64,
) -> Result<(Moment, LtbZone)> {
    section.validate()?;
    let fy_v = positive("fy", fy.to_mpa())?;
    let e_v = positive("e", e.to_mpa())?;
    let lb_v = non_negative("lb", lb.to_mm())?;
    let cb = in_range("cb", cb, 1.0, 3.0)?;

    let zx = positive("zx", section.zx.to_mm3())?;
    let sx = positive("sx", section.sx.to_mm3())?;
    let mp = zx * fy_v;

    let lp_v = lp(section, fy, e)?.to_mm();
    let lr_v = lr(section, fy, e)?.to_mm();

    if lb_v <= lp_v {
        return Ok((Moment::n_mm(mp), LtbZone::Plastic));
    }

    // 0.7*Fy*Sx is the moment at which residual stresses trigger inelastic buckling.
    let m_r = 0.7 * fy_v * sx;

    if lb_v <= lr_v {
        // F2-2: linear interpolation between Mp at Lp and 0.7*Fy*Sx at Lr.
        let interp = (lb_v - lp_v) / (lr_v - lp_v);
        let mn = cb * (mp - (mp - m_r) * interp);
        return Ok((Moment::n_mm(mn.min(mp)), LtbZone::Inelastic));
    }

    // F2-3 and F2-4: elastic critical stress.
    let rts = section.rts()?.to_mm();
    let ho = section.flange_centroid_distance().to_mm();
    let j = section.torsional_constant();
    let slenderness = lb_v / rts;
    let fcr = cb * std::f64::consts::PI.powi(2) * e_v / slenderness.powi(2)
        * (1.0 + 0.078 * j / (sx * ho) * slenderness.powi(2)).sqrt();
    let mn = fcr * sx;
    Ok((Moment::n_mm(mn.min(mp)), LtbZone::Elastic))
}

/// Nominal flexural strength limited by flange local buckling, SNI 1729 F3.
///
/// Returns `Ok(None)` when the compression flange is compact, because FLB then
/// cannot govern. For a non-compact flange it interpolates (F3-1); for a slender
/// flange it applies the plate buckling expression (F3-2) with
/// `kc = 4/sqrt(h/tw)` clamped to 0.35 to 0.76.
///
/// Units: `fy`, `e` in MPa; result in N·mm.
///
/// # Errors
///
/// Errors if the section is invalid, or `fy` or `e` are not positive and finite.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 1729.
pub fn flange_local_buckling_strength(
    section: &SteelSection,
    fy: Stress,
    e: Stress,
) -> Result<Option<Moment>> {
    let class = classify_flexural_section(section, fy, e)?;
    let fy_v = positive("fy", fy.to_mpa())?;
    let e_v = positive("e", e.to_mpa())?;
    let sx = positive("sx", section.sx.to_mm3())?;
    let zx = positive("zx", section.zx.to_mm3())?;
    let mp = zx * fy_v;

    match class.flange {
        Slenderness::Compact => Ok(None),
        Slenderness::NonCompact => {
            let interp = (class.lambda_flange - class.lambda_p_flange)
                / (class.lambda_r_flange - class.lambda_p_flange);
            let mn = mp - (mp - 0.7 * fy_v * sx) * interp;
            Ok(Some(Moment::n_mm(mn.min(mp))))
        }
        Slenderness::Slender => {
            let kc = (4.0 / class.lambda_web.sqrt()).clamp(0.35, 0.76);
            let mn = 0.9 * e_v * kc * sx / class.lambda_flange.powi(2);
            Ok(Some(Moment::n_mm(mn.min(mp))))
        }
    }
}

/// Full strong-axis flexural check of a doubly symmetric rolled I section.
///
/// Evaluates yielding (`Mp = Zx*Fy`), lateral torsional buckling (F2) and flange
/// local buckling (F3), takes the lowest as `Mn`, applies `phi_b = 0.90`, and
/// reports the demand over capacity ratio together with every intermediate value.
///
/// Units: everything in the strict SI convention, mm / MPa / N·mm.
///
/// # Errors
///
/// - [`SniError::UnsupportedSection`] when the web is not compact, because
///   clauses F4 and F5 are not implemented here.
/// - [`SniError::NonPositive`], [`SniError::Negative`], [`SniError::NotFinite`] or
///   [`SniError::OutOfRange`] for input that is not physically meaningful.
/// - [`SniError::InconsistentGeometry`] for impossible section dimensions.
///
/// # Design responsibility
///
/// This is a calculation aid, not a substitute for a licensed engineer's
/// judgement. Every result must be checked against the governing edition of
/// SNI 1729 before it is used in a real design.
///
/// # Examples
///
/// ```
/// use sni_struct::steel::{check_flexure, profiles, FlexureInput, LimitState, LtbZone};
/// use sni_struct::units::{Length, Moment, Stress};
///
/// let section = profiles::wf("WF 400x200x8x13").expect("profile exists");
/// let input = FlexureInput::new(
///     section,
///     Stress::mpa(240.0),      // BJ 37
///     Length::m(2.0),          // unbraced length
///     Moment::kn_m(180.0),     // required strength Mu
/// );
/// let check = check_flexure(&input)?;
///
/// assert_eq!(check.ltb_zone, LtbZone::Plastic);
/// assert_eq!(check.governing, LimitState::Yielding);
/// // Mp = 1 285 952 mm3 * 240 MPa = 308.63 kN.m, phi*Mn = 277.77 kN.m
/// assert!((check.phi_mn.to_kn_m() - 277.77).abs() < 0.01);
/// assert!(check.adequate);
/// # Ok::<(), sni_struct::SniError>(())
/// ```
pub fn check_flexure(input: &FlexureInput) -> Result<FlexureCheck> {
    let section = &input.section;
    section.validate()?;
    let classification = classify_flexural_section(section, input.fy, input.e)?;

    if classification.web != Slenderness::Compact {
        return Err(SniError::UnsupportedSection {
            reason: "web is not compact; SNI 1729 clauses F4/F5 are not implemented",
        });
    }

    let fy = positive("fy", input.fy.to_mpa())?;
    let mu = non_negative("mu", input.mu.to_n_mm())?;
    let zx = positive("zx", section.zx.to_mm3())?;
    let sx = positive("sx", section.sx.to_mm3())?;

    let mp = Moment::n_mm(zx * fy);
    let my = Moment::n_mm(sx * fy);

    let lp_v = lp(section, input.fy, input.e)?;
    let lr_v = lr(section, input.fy, input.e)?;
    let (mn_ltb, ltb_zone) =
        lateral_torsional_buckling_strength(section, input.fy, input.e, input.lb, input.cb)?;
    let mn_flb = flange_local_buckling_strength(section, input.fy, input.e)?;

    let mut mn = mp.min(mn_ltb);
    let mut governing = if mn_ltb < mp {
        LimitState::LateralTorsionalBuckling
    } else {
        LimitState::Yielding
    };
    if let Some(flb) = mn_flb {
        if flb < mn {
            mn = flb;
            governing = LimitState::FlangeLocalBuckling;
        }
    }

    let phi_mn = mn * PHI_B;
    let ratio = if phi_mn.to_n_mm() > 0.0 {
        mu / phi_mn.to_n_mm()
    } else {
        f64::INFINITY
    };

    Ok(FlexureCheck {
        classification,
        mp,
        my,
        lp: lp_v,
        lr: lr_v,
        lb: input.lb,
        ltb_zone,
        cb: input.cb,
        rts: section.rts()?,
        ry: section.ry()?,
        j: section.torsional_constant(),
        ho: section.flange_centroid_distance(),
        mn_ltb,
        mn_flb,
        mn,
        phi_b: PHI_B,
        phi_mn,
        mu: input.mu,
        ratio,
        governing,
        adequate: ratio <= 1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steel::profiles;
    use crate::units::{Area, SecondMoment, SectionModulus};

    fn wf400() -> SteelSection {
        profiles::wf("WF 400x200x8x13").expect("bundled profile")
    }

    const BJ37: Stress = Stress::mpa(240.0);

    #[test]
    fn lp_of_wf400_bj37() {
        // ry = 45.4805 mm, sqrt(E/Fy) = sqrt(200000/240) = 28.86751
        // Lp = 1.76 * 45.4805 * 28.86751 = 2310.7 mm
        let value = lp(&wf400(), BJ37, STEEL_E).expect("lp");
        assert!((value.to_mm() - 2310.7).abs() < 0.5, "got {value}");
    }

    #[test]
    fn lr_of_wf400_bj37() {
        // rts = 53.1911, J = 356762.7, Sx = 1.19e6, ho = 387
        // X = 356762.7/(1.19e6*387) = 7.7468e-4
        // 6.76*(168/200000)^2 = 4.7699e-6 ; sqrt(X^2 + that) = 2.3173e-3
        // sqrt(X + 2.3173e-3) = 0.055605
        // Lr = 1.95*53.1911*(200000/168)*0.055605 = 6865 mm
        let value = lr(&wf400(), BJ37, STEEL_E).expect("lr");
        assert!((value.to_mm() - 6865.0).abs() < 10.0, "got {value}");
    }

    #[test]
    fn plastic_zone_gives_full_mp() {
        let (mn, zone) =
            lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::m(2.0), 1.0)
                .expect("ltb");
        assert_eq!(zone, LtbZone::Plastic);
        // Mp = 1285952 * 240 = 308.628 kN.m
        assert!((mn.to_kn_m() - 308.628).abs() < 0.01, "got {mn}");
    }

    #[test]
    fn inelastic_zone_interpolates() {
        // Lb = 4000 mm, Lp = 2310.7, Lr = 6865.0
        // ratio = (4000-2310.7)/(6865.0-2310.7) = 0.37092
        // Mr = 0.7*240*1.19e6 = 199.92e6 N.mm
        // Mn = 308.628e6 - (308.628-199.92)e6*0.37092 = 268.31e6 N.mm
        let (mn, zone) =
            lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::m(4.0), 1.0)
                .expect("ltb");
        assert_eq!(zone, LtbZone::Inelastic);
        assert!((mn.to_kn_m() - 268.31).abs() < 0.5, "got {mn}");
    }

    #[test]
    fn elastic_zone_is_below_the_inelastic_result() {
        let (short, _) =
            lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::m(6.0), 1.0)
                .expect("ltb 6 m");
        let (long, zone) =
            lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::m(9.0), 1.0)
                .expect("ltb 9 m");
        assert_eq!(zone, LtbZone::Elastic);
        assert!(long < short);
    }

    #[test]
    fn cb_raises_but_never_above_mp() {
        let (mn, _) =
            lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::m(4.0), 2.3)
                .expect("ltb");
        // Cb = 2.3 would give 617 kN.m unclipped; the cap at Mp holds.
        assert!((mn.to_kn_m() - 308.628).abs() < 0.01, "got {mn}");
    }

    #[test]
    fn compact_flange_has_no_flb_limit() {
        let flb = flange_local_buckling_strength(&wf400(), BJ37, STEEL_E).expect("flb");
        assert!(flb.is_none());
    }

    #[test]
    fn zero_unbraced_length_is_plastic() {
        let (_, zone) =
            lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::mm(0.0), 1.0)
                .expect("ltb");
        assert_eq!(zone, LtbZone::Plastic);
    }

    #[test]
    fn negative_unbraced_length_is_rejected() {
        let err =
            lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::mm(-100.0), 1.0)
                .expect_err("must reject");
        assert!(matches!(
            err,
            SniError::Negative {
                parameter: "lb",
                ..
            }
        ));
    }

    #[test]
    fn cb_outside_range_is_rejected() {
        let err = lateral_torsional_buckling_strength(&wf400(), BJ37, STEEL_E, Length::m(4.0), 0.5)
            .expect_err("must reject");
        assert!(matches!(
            err,
            SniError::OutOfRange {
                parameter: "cb",
                ..
            }
        ));
    }

    /// A deliberately thin-webbed plate girder: h/tw far past lambda_r.
    fn slender_web_girder() -> SteelSection {
        let depth = Length::mm(1000.0);
        let bf = Length::mm(300.0);
        let tw = Length::mm(4.0);
        let tf = Length::mm(20.0);
        SteelSection {
            designation: "PG 1000x300x4x20".to_owned(),
            depth,
            flange_width: bf,
            web_thickness: tw,
            flange_thickness: tf,
            fillet_radius: Length::mm(0.0),
            area: Area::mm2(2.0 * 300.0 * 20.0 + 960.0 * 4.0),
            ix: SecondMoment::mm4(2.05e9),
            iy: SecondMoment::mm4(9.0e7),
            sx: SectionModulus::mm3(4.1e6),
            zx: SteelSection::plastic_modulus_from_geometry(depth, bf, tw, tf),
        }
    }

    #[test]
    fn slender_web_is_reported_as_unsupported() {
        let input = FlexureInput::new(
            slender_web_girder(),
            BJ37,
            Length::m(3.0),
            Moment::kn_m(500.0),
        );
        let err = check_flexure(&input).expect_err("must reject");
        assert!(matches!(err, SniError::UnsupportedSection { .. }));
    }

    #[test]
    fn check_reports_ratio_and_adequacy() {
        let input = FlexureInput::new(wf400(), BJ37, Length::m(2.0), Moment::kn_m(180.0));
        let check = check_flexure(&input).expect("check");
        // phi*Mn = 0.9 * 308.628 = 277.766 kN.m ; ratio = 180/277.766 = 0.648
        assert!((check.phi_mn.to_kn_m() - 277.766).abs() < 0.01);
        assert!((check.ratio - 0.648).abs() < 0.001);
        assert!(check.adequate);
        assert_eq!(check.governing, LimitState::Yielding);
    }

    #[test]
    fn overloaded_beam_is_reported_inadequate() {
        let input = FlexureInput::new(wf400(), BJ37, Length::m(2.0), Moment::kn_m(400.0));
        let check = check_flexure(&input).expect("check");
        assert!(!check.adequate);
        assert!(check.ratio > 1.0);
    }
}
