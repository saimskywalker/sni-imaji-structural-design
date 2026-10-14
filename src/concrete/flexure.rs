//! Flexural strength of a singly reinforced rectangular concrete section,
//! SNI 2847 (which adopts ACI 318).
//!
//! Units: mm, mm2, MPa, N·mm. See [`crate::units`].
//!
//! # Method
//!
//! The equivalent rectangular stress block of clause 22.2.2.4:
//!
//! ```text
//! a  = As*fy / (0.85*fc'*b)          depth of the stress block
//! c  = a / beta1                     depth to the neutral axis
//! Mn = As*fy*(d - a/2)               nominal moment
//! ```
//!
//! `beta1` follows clause 22.2.2.4.3, the strain limits and `phi` follow clauses
//! 21.2.1 and 21.2.2, and the reinforcement limits follow clauses 9.6.1.2 and
//! 9.3.3.1.
//!
//! # Scope
//!
//! - Singly reinforced rectangular sections only. Compression reinforcement,
//!   flanged sections and prestressing are out of scope.
//! - Tension reinforcement is assumed to be yielding, which the tension-controlled
//!   and transition checks then confirm. A compression-controlled result is
//!   reported and flagged rather than re-solved, because such a beam should be
//!   redesigned rather than accepted.

use crate::concrete::ConcreteLimitState;
use crate::error::{non_negative, positive, Result};
use crate::units::{Area, Length, Moment, Stress};

/// Modulus of elasticity of non-prestressed reinforcement, `Es = 200 000 MPa`
/// (SNI 2847 clause 20.2.2.2).
pub const REBAR_ES: Stress = Stress::mpa(200_000.0);

/// Transverse reinforcement type, which sets the compression-controlled `phi`.
///
/// SNI 2847 clause 21.2.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Confinement {
    /// Rectangular ties or stirrups. Compression-controlled `phi = 0.65`.
    #[default]
    Tied,
    /// Spirals or closed hoops meeting the clause 25.7.3 requirements.
    /// Compression-controlled `phi = 0.75`.
    Spiral,
}

impl Confinement {
    /// The compression-controlled resistance factor for this confinement.
    #[must_use]
    pub fn compression_controlled_phi(self) -> f64 {
        match self {
            Self::Tied => 0.65,
            Self::Spiral => 0.75,
        }
    }
}

/// Where the net tensile strain places the section.
///
/// SNI 2847 clause 21.2.2, in terms of the net tensile strain `eps_t` in the
/// extreme tension reinforcement at nominal strength and the yield strain
/// `eps_ty = fy/Es`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionBehaviour {
    /// `eps_t >= eps_ty + 0.003`. Ductile, `phi = 0.90`.
    TensionControlled,
    /// `eps_ty < eps_t < eps_ty + 0.003`. `phi` interpolates linearly.
    Transition,
    /// `eps_t <= eps_ty`. Brittle, the lowest `phi`. Redesign rather than accept.
    CompressionControlled,
}

/// Input to a singly reinforced rectangular beam check.
#[derive(Debug, Clone, PartialEq)]
pub struct ConcreteBeamInput {
    /// Web width `b`, mm.
    pub width: Length,
    /// Effective depth `d` to the centroid of the tension reinforcement, mm.
    pub effective_depth: Length,
    /// Area of tension reinforcement `As`, mm2.
    pub steel_area: Area,
    /// Specified compressive strength of concrete `fc'`, MPa.
    pub fc: Stress,
    /// Specified yield strength of reinforcement `fy`, MPa.
    pub fy: Stress,
    /// Modulus of elasticity of the reinforcement `Es`, MPa. Normally 200 000.
    pub es: Stress,
    /// Transverse reinforcement type.
    pub confinement: Confinement,
    /// Required flexural strength `Mu`, N·mm.
    pub mu: Moment,
}

impl ConcreteBeamInput {
    /// A tied beam with `Es = 200 000 MPa`.
    ///
    /// Units: `width` and `effective_depth` in mm, `steel_area` in mm2, `fc` and
    /// `fy` in MPa, `mu` in N·mm.
    pub fn new(
        width: Length,
        effective_depth: Length,
        steel_area: Area,
        fc: Stress,
        fy: Stress,
        mu: Moment,
    ) -> Self {
        Self {
            width,
            effective_depth,
            steel_area,
            fc,
            fy,
            es: REBAR_ES,
            confinement: Confinement::Tied,
            mu,
        }
    }

    /// Use spiral or closed hoop confinement instead of ties.
    #[must_use]
    pub fn with_confinement(mut self, confinement: Confinement) -> Self {
        self.confinement = confinement;
        self
    }

    /// Override the reinforcement modulus of elasticity, in MPa.
    #[must_use]
    pub fn with_steel_modulus(mut self, es: Stress) -> Self {
        self.es = es;
        self
    }
}

/// The full result of a concrete flexural check.
#[derive(Debug, Clone, PartialEq)]
pub struct ConcreteFlexureCheck {
    /// Stress block factor `beta1`, dimensionless.
    pub beta1: f64,
    /// Depth of the equivalent rectangular stress block `a`, mm.
    pub a: Length,
    /// Depth to the neutral axis `c = a/beta1`, mm.
    pub c: Length,
    /// Net tensile strain in the extreme tension reinforcement, `eps_t`.
    pub epsilon_t: f64,
    /// Yield strain of the reinforcement, `eps_ty = fy/Es`.
    pub epsilon_ty: f64,
    /// Strain classification of the section.
    pub behaviour: SectionBehaviour,
    /// Resistance factor `phi`, dimensionless.
    pub phi: f64,
    /// Nominal flexural strength `Mn = As*fy*(d - a/2)`, N·mm.
    pub mn: Moment,
    /// Design flexural strength `phi*Mn`, N·mm.
    pub phi_mn: Moment,
    /// Required flexural strength `Mu`, N·mm.
    pub mu: Moment,
    /// Demand over capacity, `Mu / (phi*Mn)`.
    pub ratio: f64,
    /// Minimum tension reinforcement `As,min`, mm2 (clause 9.6.1.2).
    pub as_min: Area,
    /// Maximum tension reinforcement `As,max`, mm2, the area at `eps_t = 0.004`
    /// (clause 9.3.3.1).
    pub as_max: Area,
    /// Reinforcement ratio `rho = As/(b*d)`, dimensionless.
    pub rho: f64,
    /// True when `As >= As,min`.
    pub min_reinforcement_ok: bool,
    /// True when `As <= As,max`.
    pub max_reinforcement_ok: bool,
    /// The limit state that governs the reported outcome.
    pub governing: ConcreteLimitState,
    /// True when the strength ratio and both reinforcement limits are satisfied.
    pub adequate: bool,
}

/// Stress block factor `beta1` from the concrete strength, SNI 2847 clause
/// 22.2.2.4.3.
///
/// ```text
/// fc' <= 28 MPa            beta1 = 0.85
/// 28 < fc' < 55 MPa        beta1 = 0.85 - 0.05*(fc' - 28)/7
/// fc' >= 55 MPa            beta1 = 0.65
/// ```
///
/// Units: `fc` in MPa.
///
/// # Errors
///
/// Errors if `fc` is not a positive finite number.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 2847.
///
/// # Examples
///
/// ```
/// use imaji_structural_design::concrete::beta1;
/// use imaji_structural_design::units::Stress;
///
/// assert!((beta1(Stress::mpa(25.0))? - 0.85).abs() < 1e-12);
/// // 0.85 - 0.05*(35-28)/7 = 0.80
/// assert!((beta1(Stress::mpa(35.0))? - 0.80).abs() < 1e-12);
/// assert!((beta1(Stress::mpa(60.0))? - 0.65).abs() < 1e-12);
/// # Ok::<(), imaji_structural_design::SniError>(())
/// ```
pub fn beta1(fc: Stress) -> Result<f64> {
    let fc = positive("fc", fc.to_mpa())?;
    if fc <= 28.0 {
        Ok(0.85)
    } else if fc >= 55.0 {
        Ok(0.65)
    } else {
        Ok(0.85 - 0.05 * (fc - 28.0) / 7.0)
    }
}

/// Resistance factor `phi` for a non-prestressed flexural member from its net
/// tensile strain, SNI 2847 clause 21.2.2.
///
/// ```text
/// eps_t <= eps_ty                  phi = 0.65 (tied) or 0.75 (spiral)
/// eps_ty < eps_t < eps_ty + 0.003  linear interpolation
/// eps_t >= eps_ty + 0.003          phi = 0.90
/// ```
///
/// Returns the factor and the behaviour classification.
///
/// # Errors
///
/// Errors if `epsilon_t` is not finite or negative, or if `epsilon_ty` is not a
/// positive finite number.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 2847.
pub fn strength_reduction_factor(
    epsilon_t: f64,
    epsilon_ty: f64,
    confinement: Confinement,
) -> Result<(f64, SectionBehaviour)> {
    let epsilon_t = non_negative("epsilon_t", epsilon_t)?;
    let epsilon_ty = positive("epsilon_ty", epsilon_ty)?;
    let phi_cc = confinement.compression_controlled_phi();

    if epsilon_t <= epsilon_ty {
        Ok((phi_cc, SectionBehaviour::CompressionControlled))
    } else if epsilon_t >= epsilon_ty + 0.003 {
        Ok((0.90, SectionBehaviour::TensionControlled))
    } else {
        let phi = phi_cc + (0.90 - phi_cc) * (epsilon_t - epsilon_ty) / 0.003;
        Ok((phi, SectionBehaviour::Transition))
    }
}

/// Minimum tension reinforcement for a rectangular beam, SNI 2847 clause 9.6.1.2.
///
/// `As,min = max(0.25*sqrt(fc')/fy, 1.4/fy) * bw * d`
///
/// Units: `fc`, `fy` in MPa; `width`, `effective_depth` in mm; result in mm2.
///
/// # Errors
///
/// Errors if any argument is not a positive finite number.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 2847.
pub fn minimum_reinforcement(
    fc: Stress,
    fy: Stress,
    width: Length,
    effective_depth: Length,
) -> Result<Area> {
    let fc = positive("fc", fc.to_mpa())?;
    let fy = positive("fy", fy.to_mpa())?;
    let b = positive("width", width.to_mm())?;
    let d = positive("effective_depth", effective_depth.to_mm())?;
    let rho_min = (0.25 * fc.sqrt() / fy).max(1.4 / fy);
    Ok(Area::mm2(rho_min * b * d))
}

/// Maximum tension reinforcement for a non-prestressed beam, SNI 2847 clause
/// 9.3.3.1.
///
/// The clause caps ductility rather than steel area: it requires `eps_t >= 0.004`
/// at nominal strength. With `eps_cu = 0.003`, that fixes the neutral axis at
/// `c = 3d/7`, and the corresponding steel area is
///
/// ```text
/// As,max = 0.85*fc'*b*beta1*(3d/7) / fy
/// ```
///
/// Units: `fc`, `fy` in MPa; `width`, `effective_depth` in mm; result in mm2.
///
/// # Errors
///
/// Errors if any argument is not a positive finite number.
///
/// # Design responsibility
///
/// A calculation aid, not a substitute for a licensed engineer's judgement.
/// Check against the governing edition of SNI 2847.
pub fn maximum_reinforcement(
    fc: Stress,
    fy: Stress,
    width: Length,
    effective_depth: Length,
) -> Result<Area> {
    let b1 = beta1(fc)?;
    let fc_v = positive("fc", fc.to_mpa())?;
    let fy_v = positive("fy", fy.to_mpa())?;
    let b = positive("width", width.to_mm())?;
    let d = positive("effective_depth", effective_depth.to_mm())?;
    // c/d = eps_cu/(eps_cu + eps_t,min) = 0.003/(0.003 + 0.004) = 3/7
    let c_max = 3.0 * d / 7.0;
    let a_max = b1 * c_max;
    Ok(Area::mm2(0.85 * fc_v * b * a_max / fy_v))
}

/// Full flexural check of a singly reinforced rectangular section, SNI 2847.
///
/// Units: strict SI, mm / mm2 / MPa / N·mm.
///
/// The section is reported adequate only when all three of these hold: the
/// strength ratio is at or below 1.0, `As >= As,min`, and `As <= As,max`. The
/// `governing` field names whichever of the three is binding.
///
/// # Errors
///
/// Errors if any dimension, strength or steel area is not a positive finite
/// number, if `mu` is negative or not finite, or if the resulting stress block
/// is deeper than the effective depth, which means the section cannot work as a
/// singly reinforced member.
///
/// # Design responsibility
///
/// This is a calculation aid, not a substitute for a licensed engineer's
/// judgement. Every result must be checked against the governing edition of
/// SNI 2847 before it is used in a real design.
///
/// # Examples
///
/// ```
/// use imaji_structural_design::concrete::{check_beam_flexure, ConcreteBeamInput, SectionBehaviour};
/// use imaji_structural_design::units::{Area, Length, Moment, Stress};
///
/// let input = ConcreteBeamInput::new(
///     Length::mm(300.0),        // b
///     Length::mm(500.0),        // d
///     Area::mm2(1500.0),        // As
///     Stress::mpa(25.0),        // fc'
///     Stress::mpa(400.0),       // fy
///     Moment::kn_m(200.0),      // Mu
/// );
/// let check = check_beam_flexure(&input)?;
///
/// // a = 1500*400/(0.85*25*300) = 94.118 mm, Mn = 600000*(500 - 47.059) = 271.76 kN.m
/// assert!((check.a.to_mm() - 94.1176).abs() < 1e-3);
/// assert!((check.mn.to_kn_m() - 271.765).abs() < 0.01);
/// assert_eq!(check.behaviour, SectionBehaviour::TensionControlled);
/// assert!((check.phi - 0.90).abs() < 1e-12);
/// assert!(check.adequate);
/// # Ok::<(), imaji_structural_design::SniError>(())
/// ```
pub fn check_beam_flexure(input: &ConcreteBeamInput) -> Result<ConcreteFlexureCheck> {
    let b = positive("width", input.width.to_mm())?;
    let d = positive("effective_depth", input.effective_depth.to_mm())?;
    let a_s = positive("steel_area", input.steel_area.to_mm2())?;
    let fc = positive("fc", input.fc.to_mpa())?;
    let fy = positive("fy", input.fy.to_mpa())?;
    let es = positive("es", input.es.to_mpa())?;
    let mu = non_negative("mu", input.mu.to_n_mm())?;

    let b1 = beta1(input.fc)?;
    let a = a_s * fy / (0.85 * fc * b);
    if a >= d {
        return Err(crate::error::SniError::InconsistentGeometry {
            detail: "stress block is deeper than the effective depth; the section \
                     cannot work as a singly reinforced member",
        });
    }
    let c = a / b1;

    // Extreme concrete compressive strain at nominal strength is 0.003
    // (SNI 2847 clause 22.2.2.1).
    let epsilon_t = 0.003 * (d - c) / c;
    let epsilon_ty = fy / es;
    let (phi, behaviour) = strength_reduction_factor(epsilon_t, epsilon_ty, input.confinement)?;

    let mn = Moment::n_mm(a_s * fy * (d - a / 2.0));
    let phi_mn = mn * phi;
    let ratio = if phi_mn.to_n_mm() > 0.0 {
        mu / phi_mn.to_n_mm()
    } else {
        f64::INFINITY
    };

    let as_min = minimum_reinforcement(input.fc, input.fy, input.width, input.effective_depth)?;
    let as_max = maximum_reinforcement(input.fc, input.fy, input.width, input.effective_depth)?;
    let min_reinforcement_ok = a_s >= as_min.to_mm2();
    let max_reinforcement_ok = a_s <= as_max.to_mm2();

    let governing = if !min_reinforcement_ok {
        ConcreteLimitState::MinimumReinforcement
    } else if !max_reinforcement_ok {
        ConcreteLimitState::MaximumReinforcement
    } else {
        ConcreteLimitState::FlexuralStrength
    };

    Ok(ConcreteFlexureCheck {
        beta1: b1,
        a: Length::mm(a),
        c: Length::mm(c),
        epsilon_t,
        epsilon_ty,
        behaviour,
        phi,
        mn,
        phi_mn,
        mu: input.mu,
        ratio,
        as_min,
        as_max,
        rho: a_s / (b * d),
        min_reinforcement_ok,
        max_reinforcement_ok,
        governing,
        adequate: ratio <= 1.0 && min_reinforcement_ok && max_reinforcement_ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SniError;

    fn beam(as_mm2: f64) -> ConcreteBeamInput {
        ConcreteBeamInput::new(
            Length::mm(300.0),
            Length::mm(500.0),
            Area::mm2(as_mm2),
            Stress::mpa(25.0),
            Stress::mpa(400.0),
            Moment::kn_m(200.0),
        )
    }

    #[test]
    fn beta1_covers_all_three_branches() {
        assert!((beta1(Stress::mpa(20.0)).expect("b1") - 0.85).abs() < 1e-12);
        assert!((beta1(Stress::mpa(28.0)).expect("b1") - 0.85).abs() < 1e-12);
        // 0.85 - 0.05*(30-28)/7 = 0.85 - 0.014286 = 0.835714
        assert!((beta1(Stress::mpa(30.0)).expect("b1") - 0.835_714_29).abs() < 1e-8);
        // 0.85 - 0.05*(35-28)/7 = 0.80
        assert!((beta1(Stress::mpa(35.0)).expect("b1") - 0.80).abs() < 1e-12);
        assert!((beta1(Stress::mpa(55.0)).expect("b1") - 0.65).abs() < 1e-12);
        assert!((beta1(Stress::mpa(70.0)).expect("b1") - 0.65).abs() < 1e-12);
        assert!(matches!(
            beta1(Stress::mpa(0.0)),
            Err(SniError::NonPositive { .. })
        ));
    }

    #[test]
    fn worked_example_tension_controlled() {
        // b=300, d=500, As=1500, fc'=25, fy=400
        // a  = 1500*400/(0.85*25*300) = 600000/6375 = 94.1176 mm
        // c  = 94.1176/0.85 = 110.7266 mm
        // et = 0.003*(500-110.7266)/110.7266 = 0.0105469
        // ety = 400/200000 = 0.002 ; et > 0.005 so phi = 0.90
        // Mn = 600000*(500 - 47.0588) = 271.7647e6 N.mm
        let check = check_beam_flexure(&beam(1500.0)).expect("check");
        assert!((check.a.to_mm() - 94.117_65).abs() < 1e-4);
        assert!((check.c.to_mm() - 110.726_6).abs() < 1e-4);
        assert!((check.epsilon_t - 0.010_546_9).abs() < 1e-7);
        assert!((check.epsilon_ty - 0.002).abs() < 1e-12);
        assert_eq!(check.behaviour, SectionBehaviour::TensionControlled);
        assert!((check.phi - 0.90).abs() < 1e-12);
        assert!((check.mn.to_kn_m() - 271.764_7).abs() < 1e-3);
        // phi*Mn = 244.588 kN.m ; ratio = 200/244.588 = 0.8177
        assert!((check.phi_mn.to_kn_m() - 244.588_2).abs() < 1e-3);
        assert!((check.ratio - 0.817_7).abs() < 1e-3);
        assert!(check.adequate);
        assert_eq!(check.governing, ConcreteLimitState::FlexuralStrength);
    }

    #[test]
    fn reinforcement_limits_of_the_worked_example() {
        // As,min: max(0.25*sqrt(25)/400, 1.4/400) = max(0.003125, 0.0035) = 0.0035
        //         0.0035*300*500 = 525 mm2
        // As,max: c = 3*500/7 = 214.286, a = 0.85*214.286 = 182.143
        //         0.85*25*300*182.143/400 = 2902.9 mm2
        let check = check_beam_flexure(&beam(1500.0)).expect("check");
        assert!((check.as_min.to_mm2() - 525.0).abs() < 1e-9);
        assert!((check.as_max.to_mm2() - 2902.9).abs() < 0.1);
        assert!(check.min_reinforcement_ok);
        assert!(check.max_reinforcement_ok);
        // rho = 1500/(300*500) = 0.01
        assert!((check.rho - 0.01).abs() < 1e-12);
    }

    #[test]
    fn minimum_reinforcement_switches_branch_at_high_strength() {
        // At fc' = 40: 0.25*sqrt(40)/fy = 1.5811/fy > 1.4/fy, so the sqrt branch wins.
        let as_min = minimum_reinforcement(
            Stress::mpa(40.0),
            Stress::mpa(400.0),
            Length::mm(300.0),
            Length::mm(500.0),
        )
        .expect("as_min");
        // 0.25*6.32456/400 = 0.0039528 ; *150000 = 592.9 mm2
        assert!((as_min.to_mm2() - 592.92).abs() < 0.01);
    }

    #[test]
    fn under_reinforced_beam_fails_the_minimum() {
        let check = check_beam_flexure(&beam(400.0)).expect("check");
        assert!(!check.min_reinforcement_ok);
        assert!(!check.adequate);
        assert_eq!(check.governing, ConcreteLimitState::MinimumReinforcement);
    }

    #[test]
    fn over_reinforced_beam_fails_the_maximum() {
        // As = 3200 > As,max = 2902.9
        let check = check_beam_flexure(&beam(3200.0)).expect("check");
        assert!(!check.max_reinforcement_ok);
        assert!(!check.adequate);
        assert_eq!(check.governing, ConcreteLimitState::MaximumReinforcement);
        // eps_t at As = 3200: a = 3200*400/6375 = 200.784, c = 236.216
        // et = 0.003*(500-236.216)/236.216 = 0.003350 -> transition, phi < 0.90
        assert_eq!(check.behaviour, SectionBehaviour::Transition);
        assert!(check.phi < 0.90 && check.phi > 0.65);
    }

    #[test]
    fn transition_phi_interpolates_linearly() {
        // Halfway: et = ety + 0.0015 = 0.0035 -> phi = 0.65 + 0.25*0.5 = 0.775
        let (phi, behaviour) =
            strength_reduction_factor(0.0035, 0.002, Confinement::Tied).expect("phi");
        assert_eq!(behaviour, SectionBehaviour::Transition);
        assert!((phi - 0.775).abs() < 1e-12);
    }

    #[test]
    fn spiral_confinement_lifts_the_compression_controlled_phi() {
        let (tied, _) = strength_reduction_factor(0.001, 0.002, Confinement::Tied).expect("phi");
        let (spiral, behaviour) =
            strength_reduction_factor(0.001, 0.002, Confinement::Spiral).expect("phi");
        assert_eq!(behaviour, SectionBehaviour::CompressionControlled);
        assert!((tied - 0.65).abs() < 1e-12);
        assert!((spiral - 0.75).abs() < 1e-12);
    }

    #[test]
    fn strain_exactly_at_the_tension_controlled_boundary() {
        // eps_t = eps_ty + 0.003 exactly: the clause gives the full 0.90.
        let (phi, behaviour) =
            strength_reduction_factor(0.005, 0.002, Confinement::Tied).expect("phi");
        assert_eq!(behaviour, SectionBehaviour::TensionControlled);
        assert!((phi - 0.90).abs() < 1e-12);
        // And exactly at the yield strain it is compression controlled.
        let (phi_cc, behaviour_cc) =
            strength_reduction_factor(0.002, 0.002, Confinement::Tied).expect("phi");
        assert_eq!(behaviour_cc, SectionBehaviour::CompressionControlled);
        assert!((phi_cc - 0.65).abs() < 1e-12);
    }

    #[test]
    fn negative_steel_area_is_rejected() {
        let mut input = beam(1500.0);
        input.steel_area = Area::mm2(-1500.0);
        assert!(matches!(
            check_beam_flexure(&input),
            Err(SniError::NonPositive {
                parameter: "steel_area",
                ..
            })
        ));
    }

    #[test]
    fn zero_effective_depth_is_rejected() {
        let mut input = beam(1500.0);
        input.effective_depth = Length::mm(0.0);
        assert!(matches!(
            check_beam_flexure(&input),
            Err(SniError::NonPositive {
                parameter: "effective_depth",
                ..
            })
        ));
    }

    #[test]
    fn absurd_steel_area_gives_a_geometry_error_not_a_wrong_number() {
        // As = 12000 mm2 in a 300x500 section: a = 753 mm, deeper than d.
        let mut input = beam(12_000.0);
        input.mu = Moment::kn_m(0.0);
        assert!(matches!(
            check_beam_flexure(&input),
            Err(SniError::InconsistentGeometry { .. })
        ));
    }

    #[test]
    fn higher_grade_concrete_lowers_the_stress_block_depth() {
        let mut input = beam(1500.0);
        input.fc = Stress::mpa(35.0);
        let check = check_beam_flexure(&input).expect("check");
        // a = 600000/(0.85*35*300) = 600000/8925 = 67.227 mm
        assert!((check.a.to_mm() - 67.226_9).abs() < 1e-3);
        assert!((check.beta1 - 0.80).abs() < 1e-12);
        // Mn = 600000*(500 - 33.613) = 279.83 kN.m, larger than the fc'=25 case.
        assert!((check.mn.to_kn_m() - 279.832).abs() < 0.01);
    }
}
