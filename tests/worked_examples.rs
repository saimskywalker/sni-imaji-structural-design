//! End to end worked examples with hand calculations in the comments.
//!
//! Every expected value below was computed by hand first, with the arithmetic
//! written out, so that a reader can check the library against the sums rather
//! than against itself.
//!
//! Units follow the crate convention: mm, mm2, mm3, mm4, N, N/mm, N·mm, MPa.

use imaji_structural_design::concrete::{
    check_beam_flexure, ConcreteBeamInput, ConcreteLimitState, SectionBehaviour,
};
use imaji_structural_design::deflection::{
    check_deflection, simply_supported_uniform, DeflectionLimit,
};
use imaji_structural_design::loads::{evaluate, lrfd_combinations, LoadSet};
use imaji_structural_design::seismic::{
    approximate_period, base_shear, bounded_period, CsGoverning, SeismicInput,
};
use imaji_structural_design::steel::{
    check_flexure, check_shear, profiles, FlexureInput, LimitState, LtbZone, ShearInput,
    Slenderness, WebShearCase, STEEL_E,
};
use imaji_structural_design::units::{
    Area, Force, Length, LineLoad, LoadEffect, Moment, SecondMoment, Stress,
};

// ---------------------------------------------------------------------------
// Worked example 1
//
// Simply supported steel floor beam.
//   Section  WF 350x175x7x11, BJ 41 (fy = 250 MPa), E = 200 000 MPa
//   Span     6.0 m, laterally braced at midspan, so Lb = 3.0 m
//   Loads    dead 8 kN/m, live 12 kN/m (service, unfactored)
// ---------------------------------------------------------------------------

const FY_BJ41: Stress = Stress::mpa(250.0);

fn beam_350() -> imaji_structural_design::steel::SteelSection {
    profiles::wf("WF 350x175x7x11").expect("bundled profile")
}

/// wu = 1.2*8 + 1.6*12 = 9.6 + 19.2 = 28.8 kN/m
/// Mu = wu*L^2/8 = 28.8*36/8 = 129.6 kN.m
/// Vu = wu*L/2 = 28.8*6/2 = 86.4 kN
#[test]
fn example1_factored_load_effects() {
    // Load effects here are moments in kN.m, all in one consistent unit.
    let moments = LoadSet::zero()
        // w*L^2/8 with w = 8 kN/m: 8*36/8 = 36 kN.m
        .with_dead(LoadEffect::new(36.0))
        // 12*36/8 = 54 kN.m
        .with_live(LoadEffect::new(54.0));

    let report = evaluate(&lrfd_combinations(), &moments).expect("evaluate");
    // 1.2*36 + 1.6*54 = 43.2 + 86.4 = 129.6
    assert_eq!(
        report.governing().combination.name,
        "1.2D + 1.6L + 0.5(Lr or R)"
    );
    assert!((report.governing().value.value() - 129.6).abs() < 1e-9);
}

/// Zx = 175*11*339 + 7*328^2/4 = 652 575 + 188 272 = 840 847 mm3
/// Mp = 840 847 * 250 = 210.212e6 N.mm = 210.21 kN.m
#[test]
fn example1_plastic_moment() {
    let section = beam_350();
    assert!((section.zx.to_mm3() - 840_847.0).abs() < 1e-6);

    let check = check_flexure(&FlexureInput::new(
        section,
        FY_BJ41,
        Length::m(3.0),
        Moment::kn_m(129.6),
    ))
    .expect("check");
    assert!(
        (check.mp.to_kn_m() - 210.211_75).abs() < 1e-4,
        "{}",
        check.mp
    );
}

/// ry = sqrt(9.84e6/6314) = sqrt(1558.44) = 39.477 mm
/// Lp = 1.76*39.477*sqrt(200000/250) = 1.76*39.477*28.28427 = 1965.1 mm
///
/// rts^2 = Iy*ho/(2*Sx) = 9.84e6*339/(2*775000) = 2152.1 -> rts = 46.390 mm
/// J = (2*175*11^3 + 328*7^3)/3 = (465 850 + 112 504)/3 = 192 784.7 mm4
/// X = J/(Sx*ho) = 192 784.7/(775 000*339) = 7.3376e-4
/// 6.76*(175/200000)^2 = 5.1756e-6 ; sqrt(X^2 + that) = 2.39041e-3
/// sqrt(X + 2.39041e-3) = 0.0558943
/// Lr = 1.95*46.390*(200000/175)*0.0558943 = 5778.5 mm
#[test]
fn example1_bracing_limits() {
    let section = beam_350();
    let lp = imaji_structural_design::steel::lp(&section, FY_BJ41, STEEL_E).expect("lp");
    let lr = imaji_structural_design::steel::lr(&section, FY_BJ41, STEEL_E).expect("lr");
    assert!((lp.to_mm() - 1965.1).abs() < 1.0, "Lp {lp}");
    assert!((lr.to_mm() - 5778.5).abs() < 10.0, "Lr {lr}");
}

/// Lb = 3000 mm sits between Lp = 1965.1 and Lr = 5778.5, so clause F2-2 applies.
///   interp = (3000 - 1965.1)/(5778.5 - 1965.1) = 1034.9/3813.4 = 0.27139
///   0.7*Fy*Sx = 0.7*250*775 000 = 135.625e6 N.mm
///   Mn = 210.212e6 - (210.212 - 135.625)e6 * 0.27139 = 189.97e6 N.mm
///   phi*Mn = 0.9*189.97 = 170.97 kN.m
///   ratio = 129.6/170.97 = 0.758
#[test]
fn example1_flexure_check() {
    let check = check_flexure(&FlexureInput::new(
        beam_350(),
        FY_BJ41,
        Length::m(3.0),
        Moment::kn_m(129.6),
    ))
    .expect("check");

    assert_eq!(check.classification.overall, Slenderness::Compact);
    assert_eq!(check.ltb_zone, LtbZone::Inelastic);
    assert_eq!(check.governing, LimitState::LateralTorsionalBuckling);
    assert!((check.mn.to_kn_m() - 189.97).abs() < 0.5, "Mn {}", check.mn);
    assert!(
        (check.phi_mn.to_kn_m() - 170.97).abs() < 0.5,
        "phi*Mn {}",
        check.phi_mn
    );
    assert!((check.ratio - 0.758).abs() < 0.005, "ratio {}", check.ratio);
    assert!(check.adequate);
}

/// h = 350 - 2*11 - 2*14 = 300 mm, h/tw = 300/7 = 42.857
/// 2.24*sqrt(200000/250) = 2.24*28.28427 = 63.357, so the web is stocky.
/// Aw = 350*7 = 2450 mm2
/// Vn = 0.6*250*2450*1.0 = 367 500 N = 367.5 kN, phi_v = 1.00
/// ratio = 86.4/367.5 = 0.2351
#[test]
fn example1_shear_check() {
    let check = check_shear(&ShearInput::new(beam_350(), FY_BJ41, Force::kn(86.4))).expect("check");

    assert!((check.h_over_tw - 42.857_142_9).abs() < 1e-6);
    assert_eq!(check.case, WebShearCase::StockyWebFullYield);
    assert!((check.phi_v - 1.0).abs() < 1e-12);
    assert!((check.vn.to_kn() - 367.5).abs() < 1e-9, "Vn {}", check.vn);
    assert!((check.ratio - 0.235_102).abs() < 1e-5);
    assert!(check.adequate);
}

/// Live load deflection, service load, no factors.
/// delta = 5*w*L^4/(384*E*I) with w = 12 N/mm, L = 6000 mm, I = 13 600 cm4
///       = 5*12*1.296e15 / (384*200000*1.36e8)
///       = 7.776e16 / 1.04448e16 = 7.4448 mm
/// L/360 = 16.667 mm, so ratio = 0.4467
#[test]
fn example1_live_load_deflection() {
    let delta = simply_supported_uniform(
        LineLoad::kn_per_m(12.0),
        Length::m(6.0),
        STEEL_E,
        SecondMoment::cm4(13_600.0),
    )
    .expect("delta");
    assert!((delta.to_mm() - 7.4448).abs() < 1e-3, "delta {delta}");

    let check = check_deflection(delta, Length::m(6.0), DeflectionLimit::Floor).expect("check");
    assert!((check.allowable.to_mm() - 16.666_7).abs() < 1e-3);
    assert!(
        (check.ratio - 0.446_69).abs() < 1e-4,
        "ratio {}",
        check.ratio
    );
    assert!(check.adequate);
}

/// The same beam under a limit for brittle finishes, L/480 = 12.5 mm, still passes,
/// but the total load deflection under D + L would not: with w = 20 kN/m the
/// deflection is 7.4448 * 20/12 = 12.408 mm, ratio 0.993, marginal.
#[test]
fn example1_tighter_limit_is_marginal_under_total_load() {
    let delta = simply_supported_uniform(
        LineLoad::kn_per_m(20.0),
        Length::m(6.0),
        STEEL_E,
        SecondMoment::cm4(13_600.0),
    )
    .expect("delta");
    assert!((delta.to_mm() - 12.408).abs() < 1e-2, "delta {delta}");

    let check = check_deflection(
        delta,
        Length::m(6.0),
        DeflectionLimit::SupportingBrittleElements,
    )
    .expect("check");
    assert!(check.adequate);
    assert!(check.ratio > 0.98, "ratio {}", check.ratio);
}

// ---------------------------------------------------------------------------
// Worked example 2
//
// Singly reinforced concrete beam.
//   b = 350 mm, d = 550 mm
//   fc' = 30 MPa, fy = 420 MPa
//   As = 4 D25 = 4 * 490.87 = 1963.5 mm2
//   Mu = 300 kN.m
// ---------------------------------------------------------------------------

fn concrete_beam() -> ConcreteBeamInput {
    ConcreteBeamInput::new(
        Length::mm(350.0),
        Length::mm(550.0),
        Area::mm2(1963.5),
        Stress::mpa(30.0),
        Stress::mpa(420.0),
        Moment::kn_m(300.0),
    )
}

/// beta1 = 0.85 - 0.05*(30 - 28)/7 = 0.85 - 0.0142857 = 0.8357143
/// a = As*fy/(0.85*fc'*b) = 1963.5*420/(0.85*30*350) = 824 670/8925 = 92.400 mm
/// c = a/beta1 = 92.400/0.8357143 = 110.564 mm
#[test]
fn example2_stress_block() {
    let check = check_beam_flexure(&concrete_beam()).expect("check");
    assert!((check.beta1 - 0.835_714_29).abs() < 1e-8);
    assert!((check.a.to_mm() - 92.400).abs() < 1e-3, "a {}", check.a);
    assert!((check.c.to_mm() - 110.5641).abs() < 1e-3, "c {}", check.c);
}

/// eps_ty = 420/200000 = 0.0021
/// eps_t  = 0.003*(550 - 110.5641)/110.5641 = 0.003*3.974486 = 0.0119235
/// 0.0119235 >= 0.0021 + 0.003 = 0.0051, so tension controlled and phi = 0.90
#[test]
fn example2_strain_and_phi() {
    let check = check_beam_flexure(&concrete_beam()).expect("check");
    assert!((check.epsilon_ty - 0.0021).abs() < 1e-12);
    assert!(
        (check.epsilon_t - 0.011_923_5).abs() < 1e-6,
        "eps_t {}",
        check.epsilon_t
    );
    assert_eq!(check.behaviour, SectionBehaviour::TensionControlled);
    assert!((check.phi - 0.90).abs() < 1e-12);
}

/// Mn = As*fy*(d - a/2) = 824 670*(550 - 46.200) = 824 670*503.800 = 415.4687e6 N.mm
/// phi*Mn = 0.9*415.4687 = 373.9219 kN.m
/// ratio = 300/373.9219 = 0.80231
#[test]
fn example2_moment_capacity() {
    let check = check_beam_flexure(&concrete_beam()).expect("check");
    assert!(
        (check.mn.to_kn_m() - 415.468_746).abs() < 1e-4,
        "Mn {}",
        check.mn
    );
    assert!(
        (check.phi_mn.to_kn_m() - 373.921_871).abs() < 1e-4,
        "phi*Mn {}",
        check.phi_mn
    );
    assert!(
        (check.ratio - 0.802_31).abs() < 1e-4,
        "ratio {}",
        check.ratio
    );
    assert!(check.adequate);
    assert_eq!(check.governing, ConcreteLimitState::FlexuralStrength);
}

/// As,min = max(0.25*sqrt(30)/420, 1.4/420)*350*550
///        = max(0.0032603, 0.0033333)*192 500 = 641.67 mm2
/// As,max: c = 3*550/7 = 235.7143, a = 0.8357143*235.7143 = 196.9898
///         As,max = 0.85*30*350*196.9898/420 = 1 758 133.9/420 = 4186.03 mm2
#[test]
fn example2_reinforcement_limits() {
    let check = check_beam_flexure(&concrete_beam()).expect("check");
    assert!(
        (check.as_min.to_mm2() - 641.667).abs() < 0.01,
        "As,min {}",
        check.as_min
    );
    assert!(
        (check.as_max.to_mm2() - 4186.03).abs() < 0.01,
        "As,max {}",
        check.as_max
    );
    assert!(check.min_reinforcement_ok);
    assert!(check.max_reinforcement_ok);
    // rho = 1963.5/(350*550) = 0.0102
    assert!((check.rho - 0.010_200).abs() < 1e-5);
}

// ---------------------------------------------------------------------------
// Worked example 3
//
// Seismic base shear for a five storey concrete moment frame.
//   hn = 20 m, Ct = 0.0466, x = 0.9, Cu = 1.4
//   Sds = 0.75, Sd1 = 0.45, TL = 20 s, R = 8, Ie = 1.0
//   W = 12 000 kN
// ---------------------------------------------------------------------------

/// Ta = 0.0466 * 20^0.9 = 0.0466 * 14.8227 = 0.69074 s
/// A rational analysis giving T = 1.10 s is capped at Cu*Ta = 1.4*0.69074 = 0.96704 s
#[test]
fn example3_period() {
    let ta = approximate_period(0.0466, Length::m(20.0), 0.9).expect("ta");
    assert!((ta - 0.690_74).abs() < 1e-4, "Ta {ta}");

    let t = bounded_period(1.10, 1.4, ta).expect("t");
    assert!((t - 0.967_04).abs() < 1e-4, "T {t}");
}

/// R/Ie = 8
/// Cs plateau = 0.75/8 = 0.09375
/// Cs upper   = 0.45/(0.96704*8) = 0.45/7.73632 = 0.058167  <- governs
/// Cs min     = max(0.044*0.75*1.0, 0.01) = 0.033
/// V = 0.058167 * 12 000 kN = 698.0 kN
#[test]
fn example3_base_shear() {
    let ta = approximate_period(0.0466, Length::m(20.0), 0.9).expect("ta");
    let period = bounded_period(1.10, 1.4, ta).expect("t");

    let result = base_shear(&SeismicInput {
        sds: 0.75,
        sd1: 0.45,
        s1: None,
        r: 8.0,
        ie: 1.0,
        period,
        tl: 20.0,
        weight: Force::kn(12_000.0),
    })
    .expect("base shear");

    assert!((result.cs_plateau - 0.09375).abs() < 1e-12);
    assert!(
        (result.cs_upper - 0.058_167).abs() < 1e-5,
        "Cs upper {}",
        result.cs_upper
    );
    assert!((result.cs_minimum - 0.033).abs() < 1e-12);
    assert_eq!(result.governing, CsGoverning::LongPeriodLimit);
    assert!(
        (result.base_shear.to_kn() - 698.0).abs() < 0.5,
        "V {}",
        result.base_shear
    );
}

/// The same building on a stiffer site, with the seismic effect then carried
/// through the load combinations at a column base.
///   D = 900 kN, L = 300 kN axial; E = 250 kN, W = 180 kN
///   1.2D + 1.0E + 1.0L = 1080 + 250 + 300 = 1630 kN            <- governs
///   1.2D + 1.0W + 1.0L + 0 = 1080 + 180 + 300 = 1560 kN
#[test]
fn example3_seismic_governs_at_the_column_base() {
    let axial = LoadSet::zero()
        .with_dead(LoadEffect::new(900.0))
        .with_live(LoadEffect::new(300.0))
        .with_wind(LoadEffect::new(180.0))
        .with_seismic(LoadEffect::new(250.0));

    let report = evaluate(&lrfd_combinations(), &axial).expect("evaluate");
    assert_eq!(report.governing().combination.name, "1.2D + 1.0E + 1.0L");
    assert!((report.governing().value.value() - 1630.0).abs() < 1e-9);
}

/// The counterpart the brief asks for: the same frame where wind is the larger
/// lateral effect, so combination 4 overtakes the seismic combination.
///   D = 900, L = 300, E = 150, W = 400
///   1.2D + 1.0W + 1.0L + 0 = 1080 + 400 + 300 = 1780 kN        <- governs
///   1.2D + 1.0E + 1.0L     = 1080 + 150 + 300 = 1530 kN
#[test]
fn example3_wind_governs_over_seismic() {
    let axial = LoadSet::zero()
        .with_dead(LoadEffect::new(900.0))
        .with_live(LoadEffect::new(300.0))
        .with_wind(LoadEffect::new(400.0))
        .with_seismic(LoadEffect::new(150.0));

    let report = evaluate(&lrfd_combinations(), &axial).expect("evaluate");
    assert_eq!(
        report.governing().combination.name,
        "1.2D + 1.0W + 1.0L + 0.5(Lr or R)"
    );
    assert!((report.governing().value.value() - 1780.0).abs() < 1e-9);
    // The seismic combination is combination 5 in the default list, at 1530 kN.
    assert!((report.results[5].value.value() - 1530.0).abs() < 1e-9);
}
