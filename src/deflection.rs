//! Serviceability deflection limits and elastic beam deflection formulas.
//!
//! Units: mm for lengths and deflections, N for point loads, N/mm for line loads,
//! MPa for the modulus, mm4 for the second moment. See [`crate::units`].
//!
//! # Where the limits come from
//!
//! The span over ratio limits in [`DeflectionLimit`] are the familiar
//! serviceability values of SNI 2847 clause 24.2.2, and SNI 1729 refers to
//! equivalent values for steel. They are selected by the caller from the enum
//! rather than looked up from a stored copy of the table, and the variant names
//! describe the situation each applies to. Read the clause to decide which one
//! your member falls under: the choice depends on what the member supports and on
//! whether that construction is likely to be damaged by large deflections.
//!
//! # Which deflection to compare
//!
//! The clause limits differ in *which* deflection they apply to. Some limit the
//! immediate live load deflection, others the part of the total deflection that
//! occurs after non-structural elements are attached, including long-term creep.
//! The formulas here give elastic deflections for a single load case; combining
//! them and applying long-term multipliers is the engineer's job.

use crate::error::{non_negative, positive, Result};
use crate::units::{Force, Length, LineLoad, SecondMoment, Stress};

/// A serviceability deflection limit expressed as span over a denominator.
///
/// SNI 2847 clause 24.2.2. The variant names describe the situation; check the
/// clause to confirm which applies to the member in hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeflectionLimit {
    /// `L/180`. Flat roofs not supporting or attached to non-structural elements
    /// likely to be damaged by large deflections, immediate live load deflection.
    FlatRoof,
    /// `L/360`. Floors not supporting or attached to non-structural elements
    /// likely to be damaged by large deflections, immediate live load deflection.
    Floor,
    /// `L/480`. Members supporting or attached to non-structural elements likely
    /// to be damaged by large deflections.
    SupportingBrittleElements,
    /// `L/240`. Members supporting or attached to non-structural elements not
    /// likely to be damaged by large deflections.
    SupportingDuctileElements,
    /// A project-specific limit, `L` over the supplied denominator.
    ///
    /// Use this when the specification, the authority having jurisdiction or a
    /// later edition of the standard sets a different value.
    SpanOver(f64),
}

impl DeflectionLimit {
    /// The denominator, so that the allowable deflection is `span / denominator`.
    #[must_use]
    pub fn denominator(self) -> f64 {
        match self {
            Self::FlatRoof => 180.0,
            Self::Floor => 360.0,
            Self::SupportingBrittleElements => 480.0,
            Self::SupportingDuctileElements => 240.0,
            Self::SpanOver(value) => value,
        }
    }

    /// The allowable deflection for a given span.
    ///
    /// Units: `span` in mm, result in mm.
    ///
    /// For a cantilever, SNI 2847 clause 24.2.2 takes the span as twice the clear
    /// projection, so pass `Length::mm(2.0 * projection)`.
    ///
    /// # Errors
    ///
    /// Errors if `span` or the denominator is not a positive finite number.
    pub fn allowable(self, span: Length) -> Result<Length> {
        let span = positive("span", span.to_mm())?;
        let denominator = positive("denominator", self.denominator())?;
        Ok(Length::mm(span / denominator))
    }
}

impl core::fmt::Display for DeflectionLimit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "L/{:.0}", self.denominator())
    }
}

/// The result of comparing a computed deflection with its limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeflectionCheck {
    /// Span used, mm.
    pub span: Length,
    /// The limit selected.
    pub limit: DeflectionLimit,
    /// Allowable deflection `span/denominator`, mm.
    pub allowable: Length,
    /// The computed deflection, mm.
    pub actual: Length,
    /// Demand over capacity, `actual / allowable`.
    pub ratio: f64,
    /// True when `ratio <= 1.0`.
    pub adequate: bool,
}

/// Compare a computed deflection with a span over ratio limit.
///
/// Units: `actual` and `span` in mm.
///
/// # Errors
///
/// Errors if `span` is not positive and finite, or if `actual` is negative or not
/// finite. Supply the magnitude of the deflection, not a signed value.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 2847 or SNI 1729.
///
/// # Examples
///
/// ```
/// use sni_struct::deflection::{check_deflection, DeflectionLimit};
/// use sni_struct::units::Length;
///
/// // 6 m span, computed 12 mm: L/360 allows 16.67 mm.
/// let check = check_deflection(Length::mm(12.0), Length::m(6.0), DeflectionLimit::Floor)?;
/// assert!((check.allowable.to_mm() - 16.6667).abs() < 1e-3);
/// assert!(check.adequate);
/// # Ok::<(), sni_struct::SniError>(())
/// ```
pub fn check_deflection(
    actual: Length,
    span: Length,
    limit: DeflectionLimit,
) -> Result<DeflectionCheck> {
    let actual_v = non_negative("actual", actual.to_mm())?;
    let allowable = limit.allowable(span)?;
    let ratio = actual_v / allowable.to_mm();
    Ok(DeflectionCheck {
        span,
        limit,
        allowable,
        actual,
        ratio,
        adequate: ratio <= 1.0,
    })
}

/// Maximum deflection of a simply supported beam under a uniform load,
/// `delta = 5*w*L^4 / (384*E*I)`.
///
/// Units: `w` in N/mm, `span` in mm, `e` in MPa, `i` in mm4, result in mm.
///
/// # Errors
///
/// Errors if `w` is negative or not finite, or if `span`, `e` or `i` are not
/// positive finite numbers.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
///
/// # Examples
///
/// ```
/// use sni_struct::deflection::simply_supported_uniform;
/// use sni_struct::units::{Length, LineLoad, SecondMoment, Stress};
///
/// // 10 kN/m over 6 m on a WF 400x200 (Ix = 23 700 cm4).
/// let delta = simply_supported_uniform(
///     LineLoad::kn_per_m(10.0),
///     Length::m(6.0),
///     Stress::mpa(200_000.0),
///     SecondMoment::cm4(23_700.0),
/// )?;
/// // 5*10*6000^4 / (384*200000*2.37e8) = 6.48e16/1.82016e16 = 3.560 mm
/// assert!((delta.to_mm() - 3.5601).abs() < 1e-3);
/// # Ok::<(), sni_struct::SniError>(())
/// ```
pub fn simply_supported_uniform(
    w: LineLoad,
    span: Length,
    e: Stress,
    i: SecondMoment,
) -> Result<Length> {
    let w = non_negative("w", w.to_n_per_mm())?;
    let l = positive("span", span.to_mm())?;
    let e = positive("e", e.to_mpa())?;
    let i = positive("i", i.to_mm4())?;
    Ok(Length::mm(5.0 * w * l.powi(4) / (384.0 * e * i)))
}

/// Maximum deflection of a simply supported beam under a point load at midspan,
/// `delta = P*L^3 / (48*E*I)`.
///
/// Units: `p` in N, `span` in mm, `e` in MPa, `i` in mm4, result in mm.
///
/// # Errors
///
/// Errors if `p` is negative or not finite, or if `span`, `e` or `i` are not
/// positive finite numbers.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
pub fn simply_supported_point_centre(
    p: Force,
    span: Length,
    e: Stress,
    i: SecondMoment,
) -> Result<Length> {
    let p = non_negative("p", p.to_n())?;
    let l = positive("span", span.to_mm())?;
    let e = positive("e", e.to_mpa())?;
    let i = positive("i", i.to_mm4())?;
    Ok(Length::mm(p * l.powi(3) / (48.0 * e * i)))
}

/// Deflection under a point load placed at distance `a` from the left support of
/// a simply supported beam, `delta = P*a^2*b^2 / (3*E*I*L)` with `b = L - a`.
///
/// This is the deflection *at the load*, which is not quite the maximum when the
/// load is off centre, but is within about three per cent of it and is the value
/// normally quoted. At midspan it reduces exactly to `P*L^3/(48*E*I)`.
///
/// Units: `p` in N, `a` and `span` in mm, `e` in MPa, `i` in mm4, result in mm.
///
/// # Errors
///
/// Errors if `p` is negative or not finite, if `span`, `e` or `i` are not
/// positive finite numbers, or if `a` is not within the span.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
pub fn simply_supported_point_at(
    p: Force,
    a: Length,
    span: Length,
    e: Stress,
    i: SecondMoment,
) -> Result<Length> {
    let p = non_negative("p", p.to_n())?;
    let l = positive("span", span.to_mm())?;
    let a = crate::error::in_range("a", a.to_mm(), 0.0, l)?;
    let e = positive("e", e.to_mpa())?;
    let i = positive("i", i.to_mm4())?;
    let b = l - a;
    Ok(Length::mm(p * a * a * b * b / (3.0 * e * i * l)))
}

/// Tip deflection of a cantilever under a uniform load,
/// `delta = w*L^4 / (8*E*I)`.
///
/// `span` here is the clear projection of the cantilever. When comparing against
/// [`DeflectionLimit`], pass twice this value as the span, per SNI 2847 clause
/// 24.2.2.
///
/// Units: `w` in N/mm, `span` in mm, `e` in MPa, `i` in mm4, result in mm.
///
/// # Errors
///
/// Errors if `w` is negative or not finite, or if `span`, `e` or `i` are not
/// positive finite numbers.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
pub fn cantilever_uniform(w: LineLoad, span: Length, e: Stress, i: SecondMoment) -> Result<Length> {
    let w = non_negative("w", w.to_n_per_mm())?;
    let l = positive("span", span.to_mm())?;
    let e = positive("e", e.to_mpa())?;
    let i = positive("i", i.to_mm4())?;
    Ok(Length::mm(w * l.powi(4) / (8.0 * e * i)))
}

/// Tip deflection of a cantilever under a point load at the tip,
/// `delta = P*L^3 / (3*E*I)`.
///
/// Units: `p` in N, `span` in mm, `e` in MPa, `i` in mm4, result in mm.
///
/// # Errors
///
/// Errors if `p` is negative or not finite, or if `span`, `e` or `i` are not
/// positive finite numbers.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
pub fn cantilever_point_tip(p: Force, span: Length, e: Stress, i: SecondMoment) -> Result<Length> {
    let p = non_negative("p", p.to_n())?;
    let l = positive("span", span.to_mm())?;
    let e = positive("e", e.to_mpa())?;
    let i = positive("i", i.to_mm4())?;
    Ok(Length::mm(p * l.powi(3) / (3.0 * e * i)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SniError;

    const E: Stress = Stress::mpa(200_000.0);

    fn i_wf400() -> SecondMoment {
        SecondMoment::cm4(23_700.0)
    }

    #[test]
    fn limits_have_the_expected_denominators() {
        assert!((DeflectionLimit::FlatRoof.denominator() - 180.0).abs() < 1e-12);
        assert!((DeflectionLimit::Floor.denominator() - 360.0).abs() < 1e-12);
        assert!((DeflectionLimit::SupportingDuctileElements.denominator() - 240.0).abs() < 1e-12);
        assert!((DeflectionLimit::SupportingBrittleElements.denominator() - 480.0).abs() < 1e-12);
        assert!((DeflectionLimit::SpanOver(500.0).denominator() - 500.0).abs() < 1e-12);
        assert_eq!(DeflectionLimit::Floor.to_string(), "L/360");
    }

    #[test]
    fn uniform_load_deflection_matches_hand_calculation() {
        // 5*10*6000^4 = 50*1.296e15 = 6.48e16
        // 384*200000*2.37e8 = 1.82016e16
        // delta = 3.5601 mm
        let delta =
            simply_supported_uniform(LineLoad::kn_per_m(10.0), Length::m(6.0), E, i_wf400())
                .expect("delta");
        assert!((delta.to_mm() - 3.560_13).abs() < 1e-4, "got {delta}");
    }

    #[test]
    fn central_point_load_deflection_matches_hand_calculation() {
        // 50000*6000^3/(48*200000*2.37e8) = 1.08e16/2.2752e15 = 4.7468 mm
        let delta = simply_supported_point_centre(Force::kn(50.0), Length::m(6.0), E, i_wf400())
            .expect("delta");
        assert!((delta.to_mm() - 4.746_8).abs() < 1e-3, "got {delta}");
    }

    #[test]
    fn off_centre_point_load_reduces_to_the_central_case_at_midspan() {
        let centre = simply_supported_point_centre(Force::kn(50.0), Length::m(6.0), E, i_wf400())
            .expect("centre");
        let at_mid = simply_supported_point_at(
            Force::kn(50.0),
            Length::m(3.0),
            Length::m(6.0),
            E,
            i_wf400(),
        )
        .expect("at mid");
        assert!((centre.to_mm() - at_mid.to_mm()).abs() < 1e-9);
    }

    #[test]
    fn off_centre_point_load_deflects_less_than_at_midspan() {
        let quarter = simply_supported_point_at(
            Force::kn(50.0),
            Length::m(1.5),
            Length::m(6.0),
            E,
            i_wf400(),
        )
        .expect("quarter");
        // P*a^2*b^2/(3EIL) = 50000*1500^2*4500^2/(3*200000*2.37e8*6000)
        //                  = 50000*2.25e6*2.025e7/8.532e17 = 2.278e18/8.532e17 = 2.670 mm
        assert!((quarter.to_mm() - 2.670_2).abs() < 1e-3, "got {quarter}");
    }

    #[test]
    fn cantilever_formulas_are_the_expected_multiples() {
        // Cantilever uniform = 48/5 times the simply supported uniform for the same L.
        let ss = simply_supported_uniform(LineLoad::kn_per_m(10.0), Length::m(3.0), E, i_wf400())
            .expect("ss");
        let cant = cantilever_uniform(LineLoad::kn_per_m(10.0), Length::m(3.0), E, i_wf400())
            .expect("cant");
        assert!((cant.to_mm() / ss.to_mm() - 48.0 / 5.0).abs() < 1e-9);
        // Cantilever tip point load = 16 times the simply supported central case.
        let ss_p = simply_supported_point_centre(Force::kn(20.0), Length::m(3.0), E, i_wf400())
            .expect("ss p");
        let cant_p =
            cantilever_point_tip(Force::kn(20.0), Length::m(3.0), E, i_wf400()).expect("cant p");
        assert!((cant_p.to_mm() / ss_p.to_mm() - 16.0).abs() < 1e-9);
    }

    #[test]
    fn deflection_check_reports_the_ratio() {
        // L/360 on a 6 m span allows 16.667 mm; 3.5601 mm gives ratio 0.2136.
        let delta =
            simply_supported_uniform(LineLoad::kn_per_m(10.0), Length::m(6.0), E, i_wf400())
                .expect("delta");
        let check = check_deflection(delta, Length::m(6.0), DeflectionLimit::Floor).expect("check");
        assert!((check.allowable.to_mm() - 16.666_7).abs() < 1e-3);
        assert!((check.ratio - 0.213_6).abs() < 1e-3);
        assert!(check.adequate);
    }

    #[test]
    fn tight_limit_can_fail_the_same_beam() {
        let check = check_deflection(
            Length::mm(20.0),
            Length::m(6.0),
            DeflectionLimit::SupportingBrittleElements,
        )
        .expect("check");
        // L/480 = 12.5 mm, ratio = 1.6
        assert!((check.allowable.to_mm() - 12.5).abs() < 1e-9);
        assert!((check.ratio - 1.6).abs() < 1e-9);
        assert!(!check.adequate);
    }

    #[test]
    fn zero_span_is_rejected() {
        assert!(matches!(
            simply_supported_uniform(LineLoad::kn_per_m(10.0), Length::mm(0.0), E, i_wf400()),
            Err(SniError::NonPositive {
                parameter: "span",
                ..
            })
        ));
        assert!(matches!(
            DeflectionLimit::Floor.allowable(Length::mm(0.0)),
            Err(SniError::NonPositive {
                parameter: "span",
                ..
            })
        ));
    }

    #[test]
    fn load_position_outside_the_span_is_rejected() {
        assert!(matches!(
            simply_supported_point_at(
                Force::kn(50.0),
                Length::m(7.0),
                Length::m(6.0),
                E,
                i_wf400()
            ),
            Err(SniError::OutOfRange { parameter: "a", .. })
        ));
    }
}
