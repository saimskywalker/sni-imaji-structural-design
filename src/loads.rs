//! LRFD load combinations, SNI 1727 clause 2.3.
//!
//! # Units
//!
//! Load combinations are linear in the load effects, so they carry no unit of
//! their own. Every effect in one [`LoadSet`] must be expressed in the *same*
//! unit, and the factored results come back in that unit. Use
//! [`crate::units::LoadEffect::from_force`] or
//! [`crate::units::LoadEffect::from_moment`] to pin the unit to N or N·mm.
//!
//! # Coefficients
//!
//! The default set in [`lrfd_combinations`] follows the strength design
//! combinations of SNI 1727 clause 2.3.1, which mirror ASCE 7. They are ordinary
//! numeric coefficients, restated here as data the caller can inspect and
//! replace, not a reproduction of the standard's table. When the project
//! specification, the authority having jurisdiction or a later edition of the
//! standard differs, build your own [`LoadCombination`] list and pass it to
//! [`evaluate`].
//!
//! Two points that regularly catch people out:
//!
//! - Snow does not appear. It is not a design load in Indonesia; the roof slot
//!   carries roof live load `Lr` or rain `R`.
//! - The seismic effect `E` in SNI 1726 is `E = rho*Qe +/- 0.2*Sds*D`. The vertical
//!   part `0.2*Sds*D` shifts the dead load coefficient rather than the seismic one.
//!   [`lrfd_combinations_with_vertical_seismic`] builds the shifted set.

use crate::error::{finite, Result, SniError};
use crate::units::LoadEffect;

/// The unfactored load effects entering a set of combinations.
///
/// All six must be in the same unit. Sign convention is the caller's: supply a
/// negative wind or seismic effect to check reversal and uplift.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LoadSet {
    /// Dead load effect `D`.
    pub dead: LoadEffect,
    /// Live load effect `L`.
    pub live: LoadEffect,
    /// Roof live load effect `Lr`.
    pub roof_live: LoadEffect,
    /// Rain load effect `R`.
    pub rain: LoadEffect,
    /// Wind load effect `W`.
    pub wind: LoadEffect,
    /// Seismic load effect `E`, the horizontal part `rho*Qe`.
    pub seismic: LoadEffect,
}

impl LoadSet {
    /// A set with every effect zero, to be filled in with the `with_*` methods.
    #[must_use]
    pub fn zero() -> Self {
        Self::default()
    }

    /// Set the dead load effect.
    #[must_use]
    pub fn with_dead(mut self, value: LoadEffect) -> Self {
        self.dead = value;
        self
    }

    /// Set the live load effect.
    #[must_use]
    pub fn with_live(mut self, value: LoadEffect) -> Self {
        self.live = value;
        self
    }

    /// Set the roof live load effect.
    #[must_use]
    pub fn with_roof_live(mut self, value: LoadEffect) -> Self {
        self.roof_live = value;
        self
    }

    /// Set the rain load effect.
    #[must_use]
    pub fn with_rain(mut self, value: LoadEffect) -> Self {
        self.rain = value;
        self
    }

    /// Set the seismic load effect.
    #[must_use]
    pub fn with_seismic(mut self, value: LoadEffect) -> Self {
        self.seismic = value;
        self
    }

    /// Set the wind load effect.
    #[must_use]
    pub fn with_wind(mut self, value: LoadEffect) -> Self {
        self.wind = value;
        self
    }

    /// The `(Lr or R)` slot: whichever of roof live and rain has the larger
    /// magnitude, keeping its sign.
    ///
    /// The standard brackets roof live load and rain as alternatives rather than
    /// summing them, because they act on the same surface and are not taken
    /// simultaneously at full value.
    #[must_use]
    pub fn roof_or_rain(&self) -> LoadEffect {
        if self.rain.value().abs() > self.roof_live.value().abs() {
            self.rain
        } else {
            self.roof_live
        }
    }

    /// Check that every effect is a finite number.
    ///
    /// # Errors
    ///
    /// [`SniError::NotFinite`] naming the first offending effect.
    pub fn validate(&self) -> Result<()> {
        finite("dead", self.dead.value())?;
        finite("live", self.live.value())?;
        finite("roof_live", self.roof_live.value())?;
        finite("rain", self.rain.value())?;
        finite("wind", self.wind.value())?;
        finite("seismic", self.seismic.value())?;
        Ok(())
    }
}

/// The coefficients of one combination.
///
/// `roof` multiplies the `(Lr or R)` slot, that is
/// [`LoadSet::roof_or_rain`], not roof live and rain separately.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LoadFactors {
    /// Coefficient on the dead load effect.
    pub dead: f64,
    /// Coefficient on the live load effect.
    pub live: f64,
    /// Coefficient on the `(Lr or R)` slot.
    pub roof: f64,
    /// Coefficient on the wind load effect.
    pub wind: f64,
    /// Coefficient on the seismic load effect.
    pub seismic: f64,
}

/// One named LRFD combination.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadCombination {
    /// Human readable form, for example `"1.2D + 1.6L + 0.5(Lr or R)"`.
    pub name: String,
    /// The clause the coefficients came from, for the calculation record.
    pub reference: String,
    /// The coefficients.
    pub factors: LoadFactors,
}

impl LoadCombination {
    /// Build a combination from a name, a clause reference and its coefficients.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        reference: impl Into<String>,
        factors: LoadFactors,
    ) -> Self {
        Self {
            name: name.into(),
            reference: reference.into(),
            factors,
        }
    }

    /// Apply the coefficients to a load set.
    ///
    /// # Errors
    ///
    /// [`SniError::NotFinite`] if any effect or coefficient is not finite.
    pub fn apply(&self, loads: &LoadSet) -> Result<LoadEffect> {
        loads.validate()?;
        finite("factors.dead", self.factors.dead)?;
        finite("factors.live", self.factors.live)?;
        finite("factors.roof", self.factors.roof)?;
        finite("factors.wind", self.factors.wind)?;
        finite("factors.seismic", self.factors.seismic)?;

        let value = self.factors.dead * loads.dead.value()
            + self.factors.live * loads.live.value()
            + self.factors.roof * loads.roof_or_rain().value()
            + self.factors.wind * loads.wind.value()
            + self.factors.seismic * loads.seismic.value();
        Ok(LoadEffect::new(value))
    }
}

impl core::fmt::Display for LoadCombination {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} [{}]", self.name, self.reference)
    }
}

/// One combination together with the factored effect it produces.
#[derive(Debug, Clone, PartialEq)]
pub struct CombinationResult {
    /// The combination applied.
    pub combination: LoadCombination,
    /// The factored load effect, in the unit of the [`LoadSet`].
    pub value: LoadEffect,
}

/// Every combination evaluated, with the governing ones identified.
#[derive(Debug, Clone, PartialEq)]
pub struct CombinationReport {
    /// All results, in the order the combinations were supplied.
    pub results: Vec<CombinationResult>,
    /// Index into `results` of the combination with the largest magnitude.
    pub governing_index: usize,
    /// Index into `results` of the algebraically smallest value.
    ///
    /// With a negative wind or seismic effect supplied this is the uplift or
    /// reversal case, which is why the `0.9D` combinations exist.
    pub minimum_index: usize,
}

impl CombinationReport {
    /// The governing result, the one with the largest magnitude.
    #[must_use]
    pub fn governing(&self) -> &CombinationResult {
        &self.results[self.governing_index]
    }

    /// The algebraically smallest result, the reversal or uplift case.
    #[must_use]
    pub fn minimum(&self) -> &CombinationResult {
        &self.results[self.minimum_index]
    }
}

/// The default strength design combinations, SNI 1727 clause 2.3.1.
///
/// Seven combinations, with clause 2.3.1 case 3 expanded into its two
/// alternatives, giving eight entries:
///
/// ```text
/// 1  1.4D
/// 2  1.2D + 1.6L + 0.5(Lr or R)
/// 3a 1.2D + 1.6(Lr or R) + 1.0L
/// 3b 1.2D + 1.6(Lr or R) + 0.5W
/// 4  1.2D + 1.0W + 1.0L + 0.5(Lr or R)
/// 5  1.2D + 1.0E + 1.0L
/// 6  0.9D + 1.0W
/// 7  0.9D + 1.0E
/// ```
///
/// The clause permits the `1.0L` in combinations 3a, 4 and 5 to drop to `0.5L`
/// where the unreduced live load is light and the occupancy is neither assembly
/// nor a garage. That relaxation is deliberately not applied here; apply it by
/// editing the returned list or by supplying your own.
///
/// # Examples
///
/// ```
/// use sni_struct::loads::lrfd_combinations;
///
/// let combos = lrfd_combinations();
/// assert_eq!(combos.len(), 8);
/// assert!((combos[0].factors.dead - 1.4).abs() < 1e-12);
/// ```
#[must_use]
pub fn lrfd_combinations() -> Vec<LoadCombination> {
    let clause = "SNI 1727 cl. 2.3.1";
    vec![
        LoadCombination::new(
            "1.4D",
            clause,
            LoadFactors {
                dead: 1.4,
                ..LoadFactors::default()
            },
        ),
        LoadCombination::new(
            "1.2D + 1.6L + 0.5(Lr or R)",
            clause,
            LoadFactors {
                dead: 1.2,
                live: 1.6,
                roof: 0.5,
                ..LoadFactors::default()
            },
        ),
        LoadCombination::new(
            "1.2D + 1.6(Lr or R) + 1.0L",
            clause,
            LoadFactors {
                dead: 1.2,
                live: 1.0,
                roof: 1.6,
                ..LoadFactors::default()
            },
        ),
        LoadCombination::new(
            "1.2D + 1.6(Lr or R) + 0.5W",
            clause,
            LoadFactors {
                dead: 1.2,
                roof: 1.6,
                wind: 0.5,
                ..LoadFactors::default()
            },
        ),
        LoadCombination::new(
            "1.2D + 1.0W + 1.0L + 0.5(Lr or R)",
            clause,
            LoadFactors {
                dead: 1.2,
                live: 1.0,
                roof: 0.5,
                wind: 1.0,
                ..LoadFactors::default()
            },
        ),
        LoadCombination::new(
            "1.2D + 1.0E + 1.0L",
            clause,
            LoadFactors {
                dead: 1.2,
                live: 1.0,
                seismic: 1.0,
                ..LoadFactors::default()
            },
        ),
        LoadCombination::new(
            "0.9D + 1.0W",
            clause,
            LoadFactors {
                dead: 0.9,
                wind: 1.0,
                ..LoadFactors::default()
            },
        ),
        LoadCombination::new(
            "0.9D + 1.0E",
            clause,
            LoadFactors {
                dead: 0.9,
                seismic: 1.0,
                ..LoadFactors::default()
            },
        ),
    ]
}

/// The default set with the vertical seismic effect folded into the dead load
/// coefficient.
///
/// SNI 1726 defines the seismic load effect as `E = rho*Qe +/- Ev` with
/// `Ev = 0.2*Sds*D`. Because `Ev` is proportional to the dead load, the two
/// seismic combinations become
///
/// ```text
/// 5  (1.2 + 0.2*Sds)D + rho*Qe + 1.0L
/// 7  (0.9 - 0.2*Sds)D + rho*Qe
/// ```
///
/// Supply the horizontal effect `Qe` as [`LoadSet::seismic`]; the redundancy
/// factor `rho` is applied here, so do not pre-multiply it.
///
/// `sds` is the short-period design spectral acceleration as a fraction of `g`,
/// the same quantity [`crate::seismic`] takes.
///
/// # Errors
///
/// Errors if `sds` is negative or not finite, or if `rho` is not finite.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing editions of SNI 1727 and SNI 1726.
///
/// # Examples
///
/// ```
/// use sni_struct::loads::lrfd_combinations_with_vertical_seismic;
///
/// // Sds = 0.7, rho = 1.3: seismic combos become 1.34D and 0.76D.
/// let combos = lrfd_combinations_with_vertical_seismic(0.7, 1.3)?;
/// let uplift = combos.last().expect("non-empty");
/// assert!((uplift.factors.dead - 0.76).abs() < 1e-12);
/// assert!((uplift.factors.seismic - 1.3).abs() < 1e-12);
/// # Ok::<(), sni_struct::SniError>(())
/// ```
pub fn lrfd_combinations_with_vertical_seismic(sds: f64, rho: f64) -> Result<Vec<LoadCombination>> {
    let sds = crate::error::non_negative("sds", sds)?;
    let rho = finite("rho", rho)?;
    let ev = 0.2 * sds;
    let clause = "SNI 1727 cl. 2.3.1 with SNI 1726 cl. 7.4.2";

    let mut combos = lrfd_combinations();
    combos.pop();
    combos.pop();
    combos.push(LoadCombination::new(
        format!("({:.3})D + {rho:.2}Qe + 1.0L", 1.2 + ev),
        clause,
        LoadFactors {
            dead: 1.2 + ev,
            live: 1.0,
            seismic: rho,
            ..LoadFactors::default()
        },
    ));
    combos.push(LoadCombination::new(
        format!("({:.3})D + {rho:.2}Qe", 0.9 - ev),
        clause,
        LoadFactors {
            dead: 0.9 - ev,
            seismic: rho,
            ..LoadFactors::default()
        },
    ));
    Ok(combos)
}

/// Evaluate every combination against a load set and find the governing one.
///
/// The governing combination is the one with the largest magnitude. The report
/// also names the algebraically smallest, which is the uplift or reversal case
/// when a negative wind or seismic effect is supplied.
///
/// # Errors
///
/// - [`SniError::EmptyCombinationSet`] if `combinations` is empty.
/// - [`SniError::NotFinite`] if any effect or coefficient is not finite.
///
/// # Design responsibility
///
/// This is a calculation aid, not a substitute for a licensed engineer's
/// judgement. Check the combinations against the governing edition of SNI 1727
/// and the project specification.
///
/// # Examples
///
/// ```
/// use sni_struct::loads::{evaluate, lrfd_combinations, LoadSet};
/// use sni_struct::units::LoadEffect;
///
/// // Moments in kN.m at a beam midspan.
/// let loads = LoadSet::zero()
///     .with_dead(LoadEffect::new(100.0))
///     .with_live(LoadEffect::new(80.0))
///     .with_roof_live(LoadEffect::new(20.0))
///     .with_wind(LoadEffect::new(60.0))
///     .with_seismic(LoadEffect::new(40.0));
///
/// let report = evaluate(&lrfd_combinations(), &loads)?;
/// // 1.2*100 + 1.0*60 + 1.0*80 + 0.5*20 = 270, above the seismic case at 240.
/// assert_eq!(report.governing().combination.name, "1.2D + 1.0W + 1.0L + 0.5(Lr or R)");
/// assert!((report.governing().value.value() - 270.0).abs() < 1e-9);
/// # Ok::<(), sni_struct::SniError>(())
/// ```
pub fn evaluate(combinations: &[LoadCombination], loads: &LoadSet) -> Result<CombinationReport> {
    if combinations.is_empty() {
        return Err(SniError::EmptyCombinationSet);
    }
    loads.validate()?;

    let mut results = Vec::with_capacity(combinations.len());
    for combination in combinations {
        let value = combination.apply(loads)?;
        results.push(CombinationResult {
            combination: combination.clone(),
            value,
        });
    }

    let mut governing_index = 0;
    let mut minimum_index = 0;
    for (index, result) in results.iter().enumerate() {
        if result.value.value().abs() > results[governing_index].value.value().abs() {
            governing_index = index;
        }
        if result.value.value() < results[minimum_index].value.value() {
            minimum_index = index;
        }
    }

    Ok(CombinationReport {
        results,
        governing_index,
        minimum_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LoadSet {
        LoadSet::zero()
            .with_dead(LoadEffect::new(100.0))
            .with_live(LoadEffect::new(80.0))
            .with_roof_live(LoadEffect::new(20.0))
            .with_wind(LoadEffect::new(60.0))
            .with_seismic(LoadEffect::new(40.0))
    }

    #[test]
    fn every_default_combination_matches_hand_calculation() {
        let report = evaluate(&lrfd_combinations(), &sample()).expect("evaluate");
        let values: Vec<f64> = report.results.iter().map(|r| r.value.value()).collect();
        // 1.4*100                                  = 140
        // 1.2*100 + 1.6*80 + 0.5*20                = 120 + 128 + 10 = 258
        // 1.2*100 + 1.6*20 + 1.0*80                = 120 + 32 + 80  = 232
        // 1.2*100 + 1.6*20 + 0.5*60                = 120 + 32 + 30  = 182
        // 1.2*100 + 1.0*60 + 1.0*80 + 0.5*20       = 120 + 60 + 80 + 10 = 270
        // 1.2*100 + 1.0*40 + 1.0*80                = 120 + 40 + 80  = 240
        // 0.9*100 + 1.0*60                         = 90 + 60 = 150
        // 0.9*100 + 1.0*40                         = 90 + 40 = 130
        let expected = [140.0, 258.0, 232.0, 182.0, 270.0, 240.0, 150.0, 130.0];
        for (got, want) in values.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-9, "got {got}, want {want}");
        }
    }

    #[test]
    fn wind_governs_over_seismic_when_wind_is_larger() {
        let report = evaluate(&lrfd_combinations(), &sample()).expect("evaluate");
        assert_eq!(
            report.governing().combination.name,
            "1.2D + 1.0W + 1.0L + 0.5(Lr or R)"
        );
        assert!((report.governing().value.value() - 270.0).abs() < 1e-9);
        // The seismic counterpart, combination 5, is the runner-up at 240.
        assert!((report.results[5].value.value() - 240.0).abs() < 1e-9);
        assert!(report.results[5].value.value() < report.governing().value.value());
    }

    #[test]
    fn seismic_governs_when_the_seismic_effect_is_larger() {
        let loads = sample().with_seismic(LoadEffect::new(200.0));
        let report = evaluate(&lrfd_combinations(), &loads).expect("evaluate");
        // 1.2*100 + 1.0*200 + 1.0*80 = 400
        assert_eq!(report.governing().combination.name, "1.2D + 1.0E + 1.0L");
        assert!((report.governing().value.value() - 400.0).abs() < 1e-9);
    }

    #[test]
    fn roof_slot_takes_the_larger_of_roof_live_and_rain() {
        let loads = sample().with_rain(LoadEffect::new(35.0));
        assert!((loads.roof_or_rain().value() - 35.0).abs() < 1e-12);
        let report = evaluate(&lrfd_combinations(), &loads).expect("evaluate");
        // Combination 2 becomes 120 + 128 + 0.5*35 = 265.5, not 258 plus rain.
        assert!((report.results[1].value.value() - 265.5).abs() < 1e-9);
    }

    #[test]
    fn uplift_case_is_reported_as_the_minimum() {
        // Net uplift: wind reverses the sign of the effect.
        let loads = LoadSet::zero()
            .with_dead(LoadEffect::new(50.0))
            .with_wind(LoadEffect::new(-120.0));
        let report = evaluate(&lrfd_combinations(), &loads).expect("evaluate");
        // 0.9*50 - 120 = -75, the only net tension case.
        assert_eq!(report.minimum().combination.name, "0.9D + 1.0W");
        assert!((report.minimum().value.value() + 75.0).abs() < 1e-9);
        // Governing by magnitude is the same case here, at 75 versus 1.4*50 = 70.
        assert_eq!(report.governing().combination.name, "0.9D + 1.0W");
    }

    #[test]
    fn empty_combination_set_is_an_error() {
        assert_eq!(evaluate(&[], &sample()), Err(SniError::EmptyCombinationSet));
    }

    #[test]
    fn non_finite_effect_is_rejected() {
        let loads = sample().with_wind(LoadEffect::new(f64::NAN));
        assert!(matches!(
            evaluate(&lrfd_combinations(), &loads),
            Err(SniError::NotFinite { parameter: "wind" })
        ));
    }

    #[test]
    fn vertical_seismic_shifts_the_dead_load_coefficient() {
        // Sds = 0.7 -> Ev = 0.14 -> 1.34D and 0.76D.
        let combos = lrfd_combinations_with_vertical_seismic(0.7, 1.0).expect("combos");
        assert_eq!(combos.len(), 8);
        assert!((combos[6].factors.dead - 1.34).abs() < 1e-12);
        assert!((combos[7].factors.dead - 0.76).abs() < 1e-12);

        let loads = sample();
        let report = evaluate(&combos, &loads).expect("evaluate");
        // 1.34*100 + 1.0*40 + 1.0*80 = 254
        assert!((report.results[6].value.value() - 254.0).abs() < 1e-9);
        // 0.76*100 + 1.0*40 = 116
        assert!((report.results[7].value.value() - 116.0).abs() < 1e-9);
    }

    #[test]
    fn custom_coefficients_can_replace_the_defaults() {
        let custom = vec![LoadCombination::new(
            "1.5D + 1.8L",
            "project specification",
            LoadFactors {
                dead: 1.5,
                live: 1.8,
                ..LoadFactors::default()
            },
        )];
        let report = evaluate(&custom, &sample()).expect("evaluate");
        // 1.5*100 + 1.8*80 = 294
        assert!((report.governing().value.value() - 294.0).abs() < 1e-9);
        assert_eq!(
            report.governing().combination.reference,
            "project specification"
        );
    }

    #[test]
    fn negative_sds_is_rejected() {
        assert!(matches!(
            lrfd_combinations_with_vertical_seismic(-0.1, 1.0),
            Err(SniError::Negative {
                parameter: "sds",
                ..
            })
        ));
    }
}
