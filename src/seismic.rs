//! Equivalent lateral force base shear, SNI 1726.
//!
//! Units: `Force` in N for the seismic weight and base shear, `Length` in mm for
//! building height, seconds for periods. The spectral accelerations `Sds`, `Sd1`
//! and `S1` are dimensionless fractions of `g`. See [`crate::units`].
//!
//! # No site coefficient table here
//!
//! `Sds` and `Sd1` are **inputs**. Deriving them needs the mapped values `Ss` and
//! `S1` for the site, the site class, and the site coefficients `Fa` and `Fv`,
//! all of which live in maps and tables in SNI 1726 and in the national seismic
//! hazard data. None of that is reproduced here. Obtain `Sds` and `Sd1` from the
//! site-specific seismic hazard study or the official hazard map service, then
//! pass them in.
//!
//! Likewise `R`, `Ie`, `Ct`, `x` and `Cu` are inputs read from the tables of
//! SNI 1726 clauses 4.1.2, 7.2.2 and 7.8.2. This module computes with them; it
//! does not store them.
//!
//! # Scope
//!
//! The equivalent lateral force procedure of clause 7.8 only: base shear `V`.
//! Vertical distribution, torsion, drift and the modal response spectrum method
//! are out of scope.

use crate::error::{in_range, non_negative, positive, Result};
use crate::units::{Force, Length};

/// Which of the four expressions in clause 7.8.1 produced `Cs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CsGoverning {
    /// The short period plateau, `Cs = Sds/(R/Ie)`, equation 7.8-2.
    ShortPeriodPlateau,
    /// The long period branch, `Cs = Sd1/(T*(R/Ie))`, equation 7.8-3, for `T <= TL`.
    LongPeriodLimit,
    /// The very long period branch, `Cs = Sd1*TL/(T^2*(R/Ie))`, equation 7.8-4,
    /// for `T > TL`.
    VeryLongPeriodLimit,
    /// The floor `Cs = max(0.044*Sds*Ie, 0.01)`, equation 7.8-5.
    MinimumFloor,
    /// The high seismicity floor `Cs = 0.5*S1/(R/Ie)`, equation 7.8-6, which
    /// applies only where `S1 >= 0.6`.
    HighSeismicityFloor,
}

impl core::fmt::Display for CsGoverning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text = match self {
            Self::ShortPeriodPlateau => "short period plateau, Cs = Sds/(R/Ie)",
            Self::LongPeriodLimit => "long period limit, Cs = Sd1/(T*(R/Ie))",
            Self::VeryLongPeriodLimit => "very long period limit, Cs = Sd1*TL/(T^2*(R/Ie))",
            Self::MinimumFloor => "minimum, Cs = max(0.044*Sds*Ie, 0.01)",
            Self::HighSeismicityFloor => "high seismicity floor, Cs = 0.5*S1/(R/Ie)",
        };
        f.write_str(text)
    }
}

/// Input to a base shear calculation.
///
/// Every spectral value is a fraction of `g`, dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeismicInput {
    /// Design spectral response acceleration at short periods, `Sds`, as a
    /// fraction of `g`. Obtain from the site-specific hazard data.
    pub sds: f64,
    /// Design spectral response acceleration at a one second period, `Sd1`, as a
    /// fraction of `g`. Obtain from the site-specific hazard data.
    pub sd1: f64,
    /// Mapped risk-targeted spectral acceleration at a one second period, `S1`,
    /// as a fraction of `g`.
    ///
    /// Only used for the high seismicity floor of equation 7.8-6, which applies
    /// where `S1 >= 0.6`. Pass `None` to skip that check.
    pub s1: Option<f64>,
    /// Response modification coefficient `R` for the seismic force resisting
    /// system, from SNI 1726 clause 7.2.2.
    pub r: f64,
    /// Importance factor `Ie`, from SNI 1726 clause 4.1.2.
    pub ie: f64,
    /// Fundamental period of the structure `T`, seconds.
    ///
    /// Normally `min(T_computed, Cu*Ta)`. See [`approximate_period`] and
    /// [`bounded_period`].
    pub period: f64,
    /// Long period transition period `TL`, seconds, from the SNI 1726 map.
    pub tl: f64,
    /// Effective seismic weight `W`, N (SNI 1726 clause 7.7.2).
    pub weight: Force,
}

/// The result of a base shear calculation, with every candidate `Cs` reported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseShearResult {
    /// `R/Ie`, dimensionless.
    pub r_over_ie: f64,
    /// `Cs` from the short period plateau, equation 7.8-2.
    pub cs_plateau: f64,
    /// `Cs` from the period-dependent upper limit, equation 7.8-3 or 7.8-4.
    pub cs_upper: f64,
    /// Which of the two period branches produced `cs_upper`.
    pub upper_branch: CsGoverning,
    /// `Cs` floor from equation 7.8-5.
    pub cs_minimum: f64,
    /// `Cs` floor from equation 7.8-6, present only where `S1 >= 0.6`.
    pub cs_high_seismicity: Option<f64>,
    /// The seismic response coefficient actually used.
    pub cs: f64,
    /// Which expression produced `cs`.
    pub governing: CsGoverning,
    /// Effective seismic weight `W`, N.
    pub weight: Force,
    /// Base shear `V = Cs*W`, N.
    pub base_shear: Force,
}

/// Approximate fundamental period `Ta = Ct * hn^x`, SNI 1726 clause 7.8.2.1.
///
/// `Ct` and `x` depend on the structural system and are read from the table in
/// that clause. They are inputs here rather than a stored table. Typical pairs
/// are `(0.0724, 0.8)` for steel moment frames and `(0.0466, 0.9)` for concrete
/// moment frames, but confirm against the clause for the system in hand.
///
/// Units: `hn` is the structural height, supplied as a [`Length`] and converted
/// to metres internally because the clause is written in metres. The result is in
/// seconds.
///
/// # Errors
///
/// Errors if `ct` or `hn` is not a positive finite number, or if `x` is not finite.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 1726.
///
/// # Examples
///
/// ```
/// use imaji_structural_design::seismic::approximate_period;
/// use imaji_structural_design::units::Length;
///
/// // Steel moment frame, hn = 20 m: Ta = 0.0724 * 20^0.8 = 0.0724 * 10.986 = 0.795 s
/// let ta = approximate_period(0.0724, Length::m(20.0), 0.8)?;
/// assert!((ta - 0.7954).abs() < 1e-3);
/// # Ok::<(), imaji_structural_design::SniError>(())
/// ```
pub fn approximate_period(ct: f64, hn: Length, x: f64) -> Result<f64> {
    let ct = positive("ct", ct)?;
    let hn_m = positive("hn", hn.to_m())?;
    let x = crate::error::finite("x", x)?;
    Ok(ct * hn_m.powf(x))
}

/// The period to use in the base shear, `T = min(T_computed, Cu*Ta)`.
///
/// SNI 1726 clause 7.8.2 caps the period from a rational analysis at `Cu` times
/// the approximate period. `Cu` comes from the table in that clause, keyed on
/// `Sd1`; it is an input here rather than a stored table.
///
/// Units: all periods in seconds.
///
/// # Errors
///
/// Errors if `computed_period`, `cu` or `ta` is not a positive finite number.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 1726.
pub fn bounded_period(computed_period: f64, cu: f64, ta: f64) -> Result<f64> {
    let computed = positive("computed_period", computed_period)?;
    let cu = positive("cu", cu)?;
    let ta = positive("ta", ta)?;
    Ok(computed.min(cu * ta))
}

/// Seismic base shear `V = Cs*W` by the equivalent lateral force procedure,
/// SNI 1726 clause 7.8.1.
///
/// The seismic response coefficient is
///
/// ```text
/// Cs  = Sds/(R/Ie)                                   equation 7.8-2
/// Cs <= Sd1/(T*(R/Ie))            for T <= TL        equation 7.8-3
/// Cs <= Sd1*TL/(T^2*(R/Ie))       for T > TL         equation 7.8-4
/// Cs >= max(0.044*Sds*Ie, 0.01)                      equation 7.8-5
/// Cs >= 0.5*S1/(R/Ie)             where S1 >= 0.6    equation 7.8-6
/// ```
///
/// The upper limits are applied first, then the floors, which is the order the
/// clause intends: a floor can lift `Cs` back above the period-dependent limit.
///
/// Units: `weight` in N, result in N. Spectral values are fractions of `g`.
///
/// # Errors
///
/// Errors if `sds`, `sd1`, `r`, `ie`, `period` or `tl` is not a positive finite
/// number, if `weight` is negative or not finite, or if `s1` is supplied and is
/// negative, not finite or above 3.0.
///
/// # Design responsibility
///
/// This is a calculation aid, not a substitute for a licensed engineer's
/// judgement. `Sds`, `Sd1`, `R`, `Ie` and `TL` must come from the site-specific
/// hazard data and the tables of SNI 1726, and the result must be checked against
/// the governing edition of that standard.
///
/// # Examples
///
/// ```
/// use imaji_structural_design::seismic::{base_shear, CsGoverning, SeismicInput};
/// use imaji_structural_design::units::Force;
///
/// let input = SeismicInput {
///     sds: 0.7,
///     sd1: 0.4,
///     s1: None,
///     r: 8.0,
///     ie: 1.0,
///     period: 0.8,
///     tl: 20.0,
///     weight: Force::kn(10_000.0),
/// };
/// let result = base_shear(&input)?;
///
/// // Plateau 0.7/8 = 0.0875; upper limit 0.4/(0.8*8) = 0.0625 governs;
/// // floor max(0.044*0.7*1.0, 0.01) = 0.0308 does not.
/// assert_eq!(result.governing, CsGoverning::LongPeriodLimit);
/// assert!((result.cs - 0.0625).abs() < 1e-12);
/// assert!((result.base_shear.to_kn() - 625.0).abs() < 1e-9);
/// # Ok::<(), imaji_structural_design::SniError>(())
/// ```
pub fn base_shear(input: &SeismicInput) -> Result<BaseShearResult> {
    let sds = positive("sds", input.sds)?;
    let sd1 = positive("sd1", input.sd1)?;
    let r = positive("r", input.r)?;
    let ie = positive("ie", input.ie)?;
    let period = positive("period", input.period)?;
    let tl = positive("tl", input.tl)?;
    let weight = non_negative("weight", input.weight.to_n())?;
    let s1 = match input.s1 {
        Some(value) => Some(in_range("s1", value, 0.0, 3.0)?),
        None => None,
    };

    let r_over_ie = r / ie;
    let cs_plateau = sds / r_over_ie;

    let (cs_upper, upper_branch) = if period <= tl {
        (sd1 / (period * r_over_ie), CsGoverning::LongPeriodLimit)
    } else {
        (
            sd1 * tl / (period * period * r_over_ie),
            CsGoverning::VeryLongPeriodLimit,
        )
    };

    let cs_minimum = (0.044 * sds * ie).max(0.01);
    let cs_high_seismicity = s1
        .filter(|value| *value >= 0.6)
        .map(|value| 0.5 * value / r_over_ie);

    let mut cs = cs_plateau;
    let mut governing = CsGoverning::ShortPeriodPlateau;

    if cs_upper < cs {
        cs = cs_upper;
        governing = upper_branch;
    }
    if cs_minimum > cs {
        cs = cs_minimum;
        governing = CsGoverning::MinimumFloor;
    }
    if let Some(floor) = cs_high_seismicity {
        if floor > cs {
            cs = floor;
            governing = CsGoverning::HighSeismicityFloor;
        }
    }

    Ok(BaseShearResult {
        r_over_ie,
        cs_plateau,
        cs_upper,
        upper_branch,
        cs_minimum,
        cs_high_seismicity,
        cs,
        governing,
        weight: input.weight,
        base_shear: Force::n(cs * weight),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SniError;

    fn base_input() -> SeismicInput {
        SeismicInput {
            sds: 0.7,
            sd1: 0.4,
            s1: None,
            r: 8.0,
            ie: 1.0,
            period: 0.8,
            tl: 20.0,
            weight: Force::kn(10_000.0),
        }
    }

    #[test]
    fn long_period_limit_governs_the_worked_example() {
        // Cs plateau = 0.7/8 = 0.0875
        // Cs upper   = 0.4/(0.8*8) = 0.0625  <- governs
        // Cs min     = max(0.044*0.7*1.0, 0.01) = 0.0308
        // V = 0.0625 * 10000 kN = 625 kN
        let result = base_shear(&base_input()).expect("base shear");
        assert!((result.cs_plateau - 0.0875).abs() < 1e-12);
        assert!((result.cs_upper - 0.0625).abs() < 1e-12);
        assert!((result.cs_minimum - 0.0308).abs() < 1e-12);
        assert_eq!(result.governing, CsGoverning::LongPeriodLimit);
        assert!((result.base_shear.to_kn() - 625.0).abs() < 1e-9);
    }

    #[test]
    fn short_period_plateau_governs_a_stiff_building() {
        // T = 0.3 s: upper = 0.4/(0.3*8) = 0.1667 > plateau 0.0875
        let mut input = base_input();
        input.period = 0.3;
        let result = base_shear(&input).expect("base shear");
        assert_eq!(result.governing, CsGoverning::ShortPeriodPlateau);
        assert!((result.cs - 0.0875).abs() < 1e-12);
        // V = 0.0875 * 10000 = 875 kN
        assert!((result.base_shear.to_kn() - 875.0).abs() < 1e-9);
    }

    #[test]
    fn very_long_period_branch_applies_beyond_tl() {
        // TL = 4 s, T = 6 s: Cs = 0.4*4/(36*8) = 1.6/288 = 0.005556,
        // below the floor 0.0308, so the minimum lifts it.
        let mut input = base_input();
        input.tl = 4.0;
        input.period = 6.0;
        let result = base_shear(&input).expect("base shear");
        assert_eq!(result.upper_branch, CsGoverning::VeryLongPeriodLimit);
        assert!((result.cs_upper - 0.005_555_6).abs() < 1e-7);
        assert_eq!(result.governing, CsGoverning::MinimumFloor);
        assert!((result.cs - 0.0308).abs() < 1e-12);
    }

    #[test]
    fn absolute_floor_of_one_per_cent_applies_in_low_seismicity() {
        // Sds = 0.15: 0.044*0.15*1.0 = 0.0066, below 0.01, so 0.01 is the floor.
        let mut input = base_input();
        input.sds = 0.15;
        input.sd1 = 0.06;
        input.period = 2.0;
        let result = base_shear(&input).expect("base shear");
        // plateau = 0.01875, upper = 0.06/(2*8) = 0.00375, floor = 0.01
        assert!((result.cs_minimum - 0.01).abs() < 1e-12);
        assert_eq!(result.governing, CsGoverning::MinimumFloor);
        assert!((result.cs - 0.01).abs() < 1e-12);
    }

    #[test]
    fn high_seismicity_floor_applies_only_above_point_six() {
        let mut input = base_input();
        input.s1 = Some(0.5);
        let below = base_shear(&input).expect("below");
        assert!(below.cs_high_seismicity.is_none());

        input.s1 = Some(0.8);
        input.period = 3.0;
        let above = base_shear(&input).expect("above");
        // 0.5*0.8/8 = 0.05, above both the 0.0308 floor and the 0.4/(3*8) = 0.01667 limit
        assert!((above.cs_high_seismicity.expect("floor") - 0.05).abs() < 1e-12);
        assert_eq!(above.governing, CsGoverning::HighSeismicityFloor);
        assert!((above.cs - 0.05).abs() < 1e-12);
    }

    #[test]
    fn importance_factor_scales_the_base_shear() {
        let mut input = base_input();
        input.ie = 1.5;
        input.period = 0.3;
        let result = base_shear(&input).expect("base shear");
        // R/Ie = 8/1.5 = 5.333, Cs = 0.7/5.333 = 0.13125
        assert!((result.cs - 0.131_25).abs() < 1e-9);
    }

    #[test]
    fn approximate_period_matches_hand_calculation() {
        // Steel MRF, hn = 20 m: 0.0724 * 20^0.8
        // 20^0.8 = exp(0.8*ln 20) = exp(0.8*2.99573) = exp(2.39659) = 10.9861
        // Ta = 0.79542 s
        let ta = approximate_period(0.0724, Length::m(20.0), 0.8).expect("ta");
        assert!((ta - 0.795_43).abs() < 1e-4, "got {ta}");

        // Concrete MRF, hn = 20 m: 20^0.9 = exp(0.9*2.99573) = exp(2.69616) = 14.8227
        // Ta = 0.0466 * 14.8227 = 0.69074 s
        let ta_rc = approximate_period(0.0466, Length::m(20.0), 0.9).expect("ta");
        assert!((ta_rc - 0.690_74).abs() < 1e-4, "got {ta_rc}");
    }

    #[test]
    fn bounded_period_caps_at_cu_times_ta() {
        // Ta = 0.8, Cu = 1.4 -> cap 1.12
        assert!((bounded_period(1.5, 1.4, 0.8).expect("t") - 1.12).abs() < 1e-12);
        // A computed period below the cap is kept.
        assert!((bounded_period(0.9, 1.4, 0.8).expect("t") - 0.9).abs() < 1e-12);
    }

    #[test]
    fn zero_period_is_rejected() {
        let mut input = base_input();
        input.period = 0.0;
        assert!(matches!(
            base_shear(&input),
            Err(SniError::NonPositive {
                parameter: "period",
                ..
            })
        ));
    }

    #[test]
    fn negative_weight_is_rejected() {
        let mut input = base_input();
        input.weight = Force::kn(-100.0);
        assert!(matches!(
            base_shear(&input),
            Err(SniError::Negative {
                parameter: "weight",
                ..
            })
        ));
    }

    #[test]
    fn nonsense_s1_is_rejected() {
        let mut input = base_input();
        input.s1 = Some(-0.2);
        assert!(matches!(
            base_shear(&input),
            Err(SniError::OutOfRange {
                parameter: "s1",
                ..
            })
        ));
    }
}
