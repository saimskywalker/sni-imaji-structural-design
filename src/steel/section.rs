//! Geometry and torsional properties of a doubly symmetric rolled I section.
//!
//! Units: mm, mm2, mm3, mm4, MPa, N, N·mm. See [`crate::units`].

use crate::error::{positive, Result, SniError};
use crate::units::{Area, Length, SecondMoment, SectionModulus, Stress};

/// A doubly symmetric rolled I section (WF / IWF / H profile) bending about its
/// strong axis.
///
/// All dimensional fields are in millimetres; the property fields are in the
/// catalogue-equivalent SI units. Build one by struct literal, or fetch a common
/// Indonesian profile from [`crate::steel::profiles`].
///
/// The struct is plain data and its constructor cannot fail. Every design check
/// calls [`SteelSection::validate`] first, so bad geometry surfaces as a
/// [`SniError`] from the check rather than as a wrong number.
#[derive(Debug, Clone, PartialEq)]
pub struct SteelSection {
    /// Catalogue designation, for example `"WF 400x200x8x13"`.
    pub designation: String,
    /// Overall depth `d`, measured outside to outside of the flanges.
    pub depth: Length,
    /// Flange width `bf`.
    pub flange_width: Length,
    /// Web thickness `tw`.
    pub web_thickness: Length,
    /// Flange thickness `tf`.
    pub flange_thickness: Length,
    /// Root fillet radius `r` between web and flange.
    ///
    /// Used only to obtain the clear web depth `h = d - 2*tf - 2*r` for the web
    /// slenderness ratio. Pass `Length::mm(0.0)` for a welded plate girder, which
    /// gives the conservative `h = d - 2*tf`.
    pub fillet_radius: Length,
    /// Gross cross-sectional area `A`.
    pub area: Area,
    /// Second moment of area about the strong axis, `Ix`.
    pub ix: SecondMoment,
    /// Second moment of area about the weak axis, `Iy`.
    pub iy: SecondMoment,
    /// Elastic section modulus about the strong axis, `Sx`.
    pub sx: SectionModulus,
    /// Plastic section modulus about the strong axis, `Zx`.
    pub zx: SectionModulus,
}

impl SteelSection {
    /// Check that the dimensions describe a buildable doubly symmetric I section.
    ///
    /// Returns `Ok(())` or the first problem found. Every design entry point in
    /// this crate calls this before touching the numbers.
    ///
    /// # Errors
    ///
    /// [`SniError::NonPositive`] or [`SniError::NotFinite`] for a dimension that is
    /// not a positive finite number, and [`SniError::InconsistentGeometry`] when the
    /// flanges or fillets consume the whole depth or the fillets the whole flange.
    pub fn validate(&self) -> Result<()> {
        let d = positive("depth", self.depth.to_mm())?;
        let bf = positive("flange_width", self.flange_width.to_mm())?;
        let tw = positive("web_thickness", self.web_thickness.to_mm())?;
        let tf = positive("flange_thickness", self.flange_thickness.to_mm())?;
        positive("area", self.area.to_mm2())?;
        positive("ix", self.ix.to_mm4())?;
        positive("iy", self.iy.to_mm4())?;
        positive("sx", self.sx.to_mm3())?;
        positive("zx", self.zx.to_mm3())?;

        let r = self.fillet_radius.to_mm();
        if !r.is_finite() {
            return Err(SniError::NotFinite {
                parameter: "fillet_radius",
            });
        }
        if r < 0.0 {
            return Err(SniError::Negative {
                parameter: "fillet_radius",
                value: r,
            });
        }
        if 2.0 * tf >= d {
            return Err(SniError::InconsistentGeometry {
                detail: "flanges consume the full depth (2*tf >= d)",
            });
        }
        if d - 2.0 * tf - 2.0 * r <= 0.0 {
            return Err(SniError::InconsistentGeometry {
                detail: "no clear web remains (d - 2*tf - 2*r <= 0)",
            });
        }
        if tw >= bf {
            return Err(SniError::InconsistentGeometry {
                detail: "web is not thinner than the flange width",
            });
        }
        Ok(())
    }

    /// Clear web depth `h = d - 2*tf - 2*r`, the length used for `h/tw`.
    ///
    /// Assumes [`SteelSection::validate`] has passed; it can otherwise be negative.
    #[inline]
    pub fn clear_web_depth(&self) -> Length {
        Length::mm(
            self.depth.to_mm()
                - 2.0 * self.flange_thickness.to_mm()
                - 2.0 * self.fillet_radius.to_mm(),
        )
    }

    /// Distance between flange centroids, `ho = d - tf`.
    #[inline]
    pub fn flange_centroid_distance(&self) -> Length {
        Length::mm(self.depth.to_mm() - self.flange_thickness.to_mm())
    }

    /// Radius of gyration about the weak axis, `ry = sqrt(Iy / A)`.
    ///
    /// # Errors
    ///
    /// Errors if `A` is not positive and finite.
    pub fn ry(&self) -> Result<Length> {
        let a = positive("area", self.area.to_mm2())?;
        let iy = positive("iy", self.iy.to_mm4())?;
        Ok(Length::mm((iy / a).sqrt()))
    }

    /// Web area used for shear, `Aw = d * tw` (SNI 1729 clause G2.1).
    ///
    /// Note this uses the *overall* depth, as the clause specifies for rolled
    /// I shapes, not the clear web depth.
    #[inline]
    pub fn web_area(&self) -> Area {
        Area::mm2(self.depth.to_mm() * self.web_thickness.to_mm())
    }

    /// Saint-Venant torsional constant `J`, from the thin-walled approximation
    /// `J = (1/3) * sum(b * t^3)` over the two flanges and the web.
    ///
    /// Root fillets are ignored, which understates `J` by a few per cent and so
    /// gives a slightly conservative `Lr`. Supply a catalogue `J` by computing
    /// `Lr` yourself if you need the exact value.
    #[inline]
    pub fn torsional_constant(&self) -> f64 {
        let bf = self.flange_width.to_mm();
        let tf = self.flange_thickness.to_mm();
        let tw = self.web_thickness.to_mm();
        let hw = self.depth.to_mm() - 2.0 * tf;
        (2.0 * bf * tf.powi(3) + hw * tw.powi(3)) / 3.0
    }

    /// Warping constant `Cw = Iy * ho^2 / 4`, exact for a doubly symmetric I section.
    #[inline]
    pub fn warping_constant(&self) -> f64 {
        let ho = self.flange_centroid_distance().to_mm();
        self.iy.to_mm4() * ho * ho / 4.0
    }

    /// Effective radius of gyration for lateral torsional buckling, `rts`.
    ///
    /// `rts^2 = sqrt(Iy * Cw) / Sx`, which for a doubly symmetric I section reduces
    /// exactly to `rts^2 = Iy * ho / (2 * Sx)`. SNI 1729 clause F2.
    ///
    /// # Errors
    ///
    /// Errors if `Sx` is not positive and finite.
    pub fn rts(&self) -> Result<Length> {
        let sx = positive("sx", self.sx.to_mm3())?;
        let iy = positive("iy", self.iy.to_mm4())?;
        let ho = self.flange_centroid_distance().to_mm();
        Ok(Length::mm((iy * ho / (2.0 * sx)).sqrt()))
    }

    /// Flange slenderness ratio for a rolled shape, `lambda_f = bf / (2*tf)`.
    #[inline]
    pub fn flange_slenderness(&self) -> f64 {
        self.flange_width.to_mm() / (2.0 * self.flange_thickness.to_mm())
    }

    /// Web slenderness ratio, `lambda_w = h / tw`.
    #[inline]
    pub fn web_slenderness(&self) -> f64 {
        self.clear_web_depth().to_mm() / self.web_thickness.to_mm()
    }

    /// Plastic section modulus about the strong axis computed from the plate
    /// geometry, ignoring root fillets.
    ///
    /// `Zx = bf*tf*(d - tf) + tw*(d - 2*tf)^2 / 4`
    ///
    /// Ignoring the fillets understates `Zx` by roughly one per cent for a rolled
    /// profile, so using this value is conservative. It is how the bundled
    /// catalogue fills its `zx` field, because catalogues publish `Sx` but often
    /// not `Zx`.
    #[inline]
    pub fn plastic_modulus_from_geometry(
        depth: Length,
        flange_width: Length,
        web_thickness: Length,
        flange_thickness: Length,
    ) -> SectionModulus {
        let d = depth.to_mm();
        let bf = flange_width.to_mm();
        let tw = web_thickness.to_mm();
        let tf = flange_thickness.to_mm();
        let hw = d - 2.0 * tf;
        SectionModulus::mm3(bf * tf * (d - tf) + tw * hw * hw / 4.0)
    }
}

/// Slenderness class of one plate element, or of the section as a whole.
///
/// SNI 1729 clause B4, Table B4.1b (elements in flexure). The limiting ratios
/// `lambda_p` and `lambda_r` are computed by [`classify_flexural_section`] from
/// `E` and `Fy`, not read from a stored table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slenderness {
    /// `lambda <= lambda_p`. Can reach the plastic moment.
    Compact,
    /// `lambda_p < lambda <= lambda_r`. Yields before local buckling but cannot
    /// sustain the plastic moment through a large rotation.
    NonCompact,
    /// `lambda > lambda_r`. Local buckling occurs before yielding.
    Slender,
}

impl Slenderness {
    /// Classify a ratio against its two limits.
    #[inline]
    fn from_limits(lambda: f64, lambda_p: f64, lambda_r: f64) -> Self {
        if lambda <= lambda_p {
            Self::Compact
        } else if lambda <= lambda_r {
            Self::NonCompact
        } else {
            Self::Slender
        }
    }

    /// The less favourable of two classes.
    #[inline]
    fn worse(self, other: Self) -> Self {
        match (self, other) {
            (Self::Slender, _) | (_, Self::Slender) => Self::Slender,
            (Self::NonCompact, _) | (_, Self::NonCompact) => Self::NonCompact,
            _ => Self::Compact,
        }
    }
}

/// Result of classifying an I section in flexure.
///
/// Carries the ratios and both limits for each element so the classification can
/// be checked by hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionClassification {
    /// `bf / (2*tf)`.
    pub lambda_flange: f64,
    /// Compact limit for the flange, `0.38 * sqrt(E/Fy)`.
    pub lambda_p_flange: f64,
    /// Non-compact limit for the flange, `1.0 * sqrt(E/Fy)`.
    pub lambda_r_flange: f64,
    /// Class of the compression flange.
    pub flange: Slenderness,
    /// `h / tw`.
    pub lambda_web: f64,
    /// Compact limit for the web, `3.76 * sqrt(E/Fy)`.
    pub lambda_p_web: f64,
    /// Non-compact limit for the web, `5.70 * sqrt(E/Fy)`.
    pub lambda_r_web: f64,
    /// Class of the web.
    pub web: Slenderness,
    /// The less favourable of the flange and web classes.
    pub overall: Slenderness,
}

/// Classify a rolled I section in flexure per SNI 1729 clause B4 (Table B4.1b).
///
/// The limits are expressions in `E` and `Fy`, evaluated here, so no table from
/// the standard is reproduced:
///
/// - flange, rolled I in flexure: `lambda_p = 0.38*sqrt(E/Fy)`, `lambda_r = 1.0*sqrt(E/Fy)`
/// - web in flexure: `lambda_p = 3.76*sqrt(E/Fy)`, `lambda_r = 5.70*sqrt(E/Fy)`
///
/// Units: `fy` and `e` in MPa.
///
/// # Errors
///
/// Errors if the section geometry fails [`SteelSection::validate`], or if `fy` or
/// `e` are not positive finite numbers.
///
/// # Design responsibility
///
/// This is a calculation aid, not a substitute for a licensed engineer's
/// judgement. Check the result against the governing edition of SNI 1729.
///
/// # Examples
///
/// ```
/// use imaji_structural_design::steel::{classify_flexural_section, profiles, Slenderness, STEEL_E};
/// use imaji_structural_design::units::Stress;
///
/// let section = profiles::wf("WF 400x200x8x13").expect("profile exists");
/// let class = classify_flexural_section(&section, Stress::mpa(240.0), STEEL_E)?;
/// assert_eq!(class.overall, Slenderness::Compact);
/// # Ok::<(), imaji_structural_design::SniError>(())
/// ```
pub fn classify_flexural_section(
    section: &SteelSection,
    fy: Stress,
    e: Stress,
) -> Result<SectionClassification> {
    section.validate()?;
    let fy = positive("fy", fy.to_mpa())?;
    let e = positive("e", e.to_mpa())?;
    let sqrt_e_fy = (e / fy).sqrt();

    let lambda_flange = section.flange_slenderness();
    let lambda_p_flange = 0.38 * sqrt_e_fy;
    let lambda_r_flange = sqrt_e_fy;
    let flange = Slenderness::from_limits(lambda_flange, lambda_p_flange, lambda_r_flange);

    let lambda_web = section.web_slenderness();
    let lambda_p_web = 3.76 * sqrt_e_fy;
    let lambda_r_web = 5.70 * sqrt_e_fy;
    let web = Slenderness::from_limits(lambda_web, lambda_p_web, lambda_r_web);

    Ok(SectionClassification {
        lambda_flange,
        lambda_p_flange,
        lambda_r_flange,
        flange,
        lambda_web,
        lambda_p_web,
        lambda_r_web,
        web,
        overall: flange.worse(web),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steel::profiles;
    use crate::steel::STEEL_E;

    fn wf400() -> SteelSection {
        profiles::wf("WF 400x200x8x13").expect("bundled profile")
    }

    #[test]
    fn geometry_of_wf400x200() {
        let s = wf400();
        // h = 400 - 2*13 - 2*16 = 342 mm
        assert!((s.clear_web_depth().to_mm() - 342.0).abs() < 1e-9);
        // ho = 400 - 13 = 387 mm
        assert!((s.flange_centroid_distance().to_mm() - 387.0).abs() < 1e-9);
        // Aw = 400 * 8 = 3200 mm2
        assert!((s.web_area().to_mm2() - 3200.0).abs() < 1e-9);
        // lambda_f = 200/(2*13) = 7.6923, lambda_w = 342/8 = 42.75
        assert!((s.flange_slenderness() - 7.692_307_692).abs() < 1e-6);
        assert!((s.web_slenderness() - 42.75).abs() < 1e-9);
    }

    #[test]
    fn ry_and_rts_of_wf400x200() {
        let s = wf400();
        // ry = sqrt(17.4e6 / 8412) = sqrt(2068.47) = 45.480 mm
        assert!((s.ry().expect("ry").to_mm() - 45.4805).abs() < 1e-3);
        // rts^2 = Iy*ho/(2*Sx) = 17.4e6*387/(2*1.19e6) = 2829.29 -> rts = 53.191 mm
        assert!((s.rts().expect("rts").to_mm() - 53.1911).abs() < 1e-3);
    }

    #[test]
    fn torsional_constant_of_wf400x200() {
        let s = wf400();
        // J = (2*200*13^3 + 374*8^3)/3 = (878800 + 191488)/3 = 356762.67 mm4
        assert!((s.torsional_constant() - 356_762.667).abs() < 1e-2);
    }

    #[test]
    fn plastic_modulus_from_geometry_matches_catalogue() {
        // Zx = 200*13*387 + 8*374^2/4 = 1006200 + 279752 = 1285952 mm3
        let zx = SteelSection::plastic_modulus_from_geometry(
            Length::mm(400.0),
            Length::mm(200.0),
            Length::mm(8.0),
            Length::mm(13.0),
        );
        assert!((zx.to_mm3() - 1_285_952.0).abs() < 1e-6);
    }

    #[test]
    fn wf400_is_compact_in_bj37() {
        let class =
            classify_flexural_section(&wf400(), Stress::mpa(240.0), STEEL_E).expect("class");
        // sqrt(200000/240) = 28.8675; lambda_pf = 10.9697, lambda_pw = 108.54
        assert!((class.lambda_p_flange - 10.969_65).abs() < 1e-4);
        assert!((class.lambda_p_web - 108.541_6).abs() < 1e-3);
        assert_eq!(class.flange, Slenderness::Compact);
        assert_eq!(class.web, Slenderness::Compact);
        assert_eq!(class.overall, Slenderness::Compact);
    }

    #[test]
    fn validate_rejects_impossible_geometry() {
        let mut s = wf400();
        s.flange_thickness = Length::mm(210.0);
        assert!(matches!(
            s.validate(),
            Err(SniError::InconsistentGeometry { .. })
        ));
    }

    #[test]
    fn validate_rejects_negative_area() {
        let mut s = wf400();
        s.area = Area::mm2(-8412.0);
        assert!(matches!(s.validate(), Err(SniError::NonPositive { .. })));
    }
}
