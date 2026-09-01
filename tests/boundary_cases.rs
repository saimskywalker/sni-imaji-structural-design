//! Edge cases: values sitting exactly on a clause boundary, degenerate geometry,
//! and input the library must refuse rather than silently mis-handle.

use sni_struct::concrete::{check_beam_flexure, ConcreteBeamInput};
use sni_struct::deflection::{simply_supported_uniform, DeflectionLimit};
use sni_struct::loads::{evaluate, lrfd_combinations, LoadSet};
use sni_struct::steel::{
    check_flexure, check_shear, classify_flexural_section, lateral_torsional_buckling_strength, lp,
    lr, profiles, FlexureInput, LimitState, LtbZone, ShearInput, Slenderness, SteelSection,
    WebShearCase, STEEL_E,
};
use sni_struct::units::{
    Area, Force, Length, LineLoad, LoadEffect, Moment, SecondMoment, SectionModulus, Stress,
};
use sni_struct::SniError;

const FY: Stress = Stress::mpa(240.0);

/// Build a welded I section from plate dimensions alone, computing every property
/// from first principles so the test carries no catalogue numbers:
///
/// ```text
/// hw = d - 2*tf
/// A  = 2*bf*tf + hw*tw
/// Ix = (bf*d^3 - (bf - tw)*hw^3)/12          Sx = 2*Ix/d
/// Iy = (2*tf*bf^3 + hw*tw^3)/12
/// ```
fn plate_girder(d: f64, bf: f64, tw: f64, tf: f64) -> SteelSection {
    let hw = d - 2.0 * tf;
    let ix = (bf * d.powi(3) - (bf - tw) * hw.powi(3)) / 12.0;
    let iy = (2.0 * tf * bf.powi(3) + hw * tw.powi(3)) / 12.0;
    let depth = Length::mm(d);
    let flange_width = Length::mm(bf);
    let web_thickness = Length::mm(tw);
    let flange_thickness = Length::mm(tf);
    SteelSection {
        designation: format!("PG {d}x{bf}x{tw}x{tf}"),
        depth,
        flange_width,
        web_thickness,
        flange_thickness,
        fillet_radius: Length::mm(0.0),
        area: Area::mm2(2.0 * bf * tf + hw * tw),
        ix: SecondMoment::mm4(ix),
        iy: SecondMoment::mm4(iy),
        sx: SectionModulus::mm3(2.0 * ix / d),
        zx: SteelSection::plastic_modulus_from_geometry(
            depth,
            flange_width,
            web_thickness,
            flange_thickness,
        ),
    }
}

// ---------------------------------------------------------------------------
// Slenderness boundaries
// ---------------------------------------------------------------------------

/// A flange whose ratio sits exactly on lambda_p is compact: the clause limit is
/// inclusive.
///
/// The dimensions are chosen so the ratio is exact in binary floating point:
/// with `tf = 8`, `2*tf = 16` is a power of two, so `bf/(2*tf)` for
/// `bf = 16*lambda_p` reproduces `lambda_p` bit for bit.
#[test]
fn flange_exactly_at_the_compact_limit_is_compact() {
    let lambda_p = 0.38 * (STEEL_E.to_mpa() / FY.to_mpa()).sqrt();
    let section = plate_girder(500.0, 16.0 * lambda_p, 12.0, 8.0);

    let class = classify_flexural_section(&section, FY, STEEL_E).expect("classify");
    assert_eq!(class.lambda_flange, class.lambda_p_flange);
    assert_eq!(class.flange, Slenderness::Compact);
}

/// A hair over the limit tips it into non-compact.
#[test]
fn flange_just_over_the_compact_limit_is_non_compact() {
    let lambda_p = 0.38 * (STEEL_E.to_mpa() / FY.to_mpa()).sqrt();
    let section = plate_girder(500.0, 16.0 * lambda_p * 1.000_001, 12.0, 8.0);

    let class = classify_flexural_section(&section, FY, STEEL_E).expect("classify");
    assert!(class.lambda_flange > class.lambda_p_flange);
    assert_eq!(class.flange, Slenderness::NonCompact);
}

/// The same inclusivity on the web limit.
#[test]
fn web_exactly_at_the_compact_limit_is_compact() {
    let lambda_p = 3.76 * (STEEL_E.to_mpa() / FY.to_mpa()).sqrt();
    // tw = 8 so h/tw is exact; h = d - 2*tf with tf = 8 and r = 0.
    let h = 8.0 * lambda_p;
    let section = plate_girder(h + 16.0, 200.0, 8.0, 8.0);

    let class = classify_flexural_section(&section, FY, STEEL_E).expect("classify");
    assert_eq!(class.lambda_web, class.lambda_p_web);
    assert_eq!(class.web, Slenderness::Compact);
}

/// A hair over and clause F2 no longer applies, so the check refuses rather than
/// quietly returning an F2 answer for an F4 section.
#[test]
fn web_just_over_the_compact_limit_is_refused_by_check_flexure() {
    let lambda_p = 3.76 * (STEEL_E.to_mpa() / FY.to_mpa()).sqrt();
    let h = 8.0 * lambda_p * 1.000_001;
    let section = plate_girder(h + 16.0, 200.0, 8.0, 8.0);

    let class = classify_flexural_section(&section, FY, STEEL_E).expect("classify");
    assert_eq!(class.web, Slenderness::NonCompact);

    let err = check_flexure(&FlexureInput::new(
        section,
        FY,
        Length::m(2.0),
        Moment::kn_m(100.0),
    ))
    .expect_err("must refuse");
    assert!(matches!(err, SniError::UnsupportedSection { .. }));
}

// ---------------------------------------------------------------------------
// Unbraced length boundaries
// ---------------------------------------------------------------------------

/// `Lb` exactly equal to `Lp` still gives the full plastic moment.
#[test]
fn unbraced_length_exactly_at_lp_is_the_plastic_zone() {
    let section = profiles::wf("WF 400x200x8x13").expect("profile");
    let lp_value = lp(&section, FY, STEEL_E).expect("lp");

    let (mn, zone) =
        lateral_torsional_buckling_strength(&section, FY, STEEL_E, lp_value, 1.0).expect("ltb");
    assert_eq!(zone, LtbZone::Plastic);
    // Mp = 1 285 952 * 240 = 308.628 kN.m
    assert!((mn.to_kn_m() - 308.628_48).abs() < 1e-4, "{mn}");
}

/// `Lb` exactly equal to `Lr` lands at the top of the inelastic branch, where the
/// interpolation collapses to exactly `0.7*Fy*Sx`.
#[test]
fn unbraced_length_exactly_at_lr_gives_seven_tenths_fy_sx() {
    let section = profiles::wf("WF 400x200x8x13").expect("profile");
    let lr_value = lr(&section, FY, STEEL_E).expect("lr");

    let (mn, zone) =
        lateral_torsional_buckling_strength(&section, FY, STEEL_E, lr_value, 1.0).expect("ltb");
    assert_eq!(zone, LtbZone::Inelastic);
    // 0.7*240*1 190 000 = 199.92e6 N.mm
    assert!((mn.to_kn_m() - 199.92).abs() < 1e-6, "{mn}");
}

/// Just past `Lr` the elastic branch takes over, and the two branches very nearly
/// meet, as they must for the capacity curve to be continuous.
///
/// They do not meet exactly, and that is a property of the standard rather than a
/// defect here. Inverting equation F2-4 for the length at which `Fcr = 0.7*Fy`
/// gives leading constants of 1.94944 and 6.7495; equation F2-6 rounds them to
/// 1.95 and 6.76. The rounded `Lr` is a shade longer than the exact crossover, so
/// the elastic branch starts about a tenth of a per cent below the end of the
/// inelastic branch. The step is conservative, and well inside any real tolerance
/// on `J` or `Fy`.
#[test]
fn the_two_ltb_branches_meet_at_lr() {
    let section = profiles::wf("WF 400x200x8x13").expect("profile");
    let lr_value = lr(&section, FY, STEEL_E).expect("lr");

    let (at_lr, _) =
        lateral_torsional_buckling_strength(&section, FY, STEEL_E, lr_value, 1.0).expect("at");
    let (past_lr, zone) = lateral_torsional_buckling_strength(
        &section,
        FY,
        STEEL_E,
        Length::mm(lr_value.to_mm() * 1.000_01),
        1.0,
    )
    .expect("past");

    assert_eq!(zone, LtbZone::Elastic);
    let relative_gap = (at_lr.to_n_mm() - past_lr.to_n_mm()).abs() / at_lr.to_n_mm();
    assert!(relative_gap < 0.005, "gap {relative_gap}");
    // And the step is downwards, that is conservative, never upwards.
    assert!(past_lr <= at_lr);
}

// ---------------------------------------------------------------------------
// Flange local buckling actually governing
// ---------------------------------------------------------------------------

/// A wide, thin-flanged welded girder: 500 x 400 x 12 x 10.
///
/// lambda_f = 400/20 = 20, between lambda_pf = 10.9697 and lambda_rf = 28.8675,
/// so the flange is non-compact and clause F3 governs.
///
/// Ix = (400*500^3 - 388*480^3)/12 = 590 858 667 mm4, Sx = 2 363 434.7 mm3
/// Zx = 400*10*490 + 12*480^2/4 = 1 960 000 + 691 200 = 2 651 200 mm3
/// Mp = 2 651 200*240 = 636.288e6 N.mm
/// Mr = 0.7*240*2 363 434.7 = 397.057e6 N.mm
/// interp = (20 - 10.9697)/(28.8675 - 10.9697) = 9.0303/17.8979 = 0.504549
/// Mn = 636.288e6 - (636.288 - 397.057)e6*0.504549 = 515.584e6 N.mm
#[test]
fn non_compact_flange_makes_flb_govern() {
    let section = plate_girder(500.0, 400.0, 12.0, 10.0);
    assert!((section.sx.to_mm3() - 2_363_434.667).abs() < 1e-2);
    assert!((section.zx.to_mm3() - 2_651_200.0).abs() < 1e-6);

    let check = check_flexure(&FlexureInput::new(
        section,
        FY,
        Length::m(2.0),
        Moment::kn_m(300.0),
    ))
    .expect("check");

    assert_eq!(check.classification.flange, Slenderness::NonCompact);
    assert_eq!(check.classification.web, Slenderness::Compact);
    // The section is short enough to be in the plastic LTB zone, so FLB is the
    // only thing that can pull Mn below Mp.
    assert_eq!(check.ltb_zone, LtbZone::Plastic);
    assert_eq!(check.governing, LimitState::FlangeLocalBuckling);
    assert!((check.mp.to_kn_m() - 636.288).abs() < 1e-3);
    assert!(
        (check.mn_flb.expect("flb limit").to_kn_m() - 515.584).abs() < 0.01,
        "{:?}",
        check.mn_flb
    );
    assert!((check.mn.to_kn_m() - 515.584).abs() < 0.01);
}

// ---------------------------------------------------------------------------
// Shear boundary
// ---------------------------------------------------------------------------

/// Just inside the stocky web limit the resistance factor is 1.00; just outside it
/// drops to 0.90 while `Cv` is still 1.0, so the design strength steps down by
/// exactly ten per cent across the boundary.
#[test]
fn shear_resistance_factor_steps_at_the_stocky_web_limit() {
    let stocky_limit = 2.24 * (STEEL_E.to_mpa() / FY.to_mpa()).sqrt();
    let tw = 8.0;

    let inside = plate_girder(stocky_limit * tw * 0.999 + 16.0, 200.0, tw, 8.0);
    let outside = plate_girder(stocky_limit * tw * 1.001 + 16.0, 200.0, tw, 8.0);

    let a = check_shear(&ShearInput::new(inside, FY, Force::kn(100.0))).expect("inside");
    let b = check_shear(&ShearInput::new(outside, FY, Force::kn(100.0))).expect("outside");

    assert_eq!(a.case, WebShearCase::StockyWebFullYield);
    assert_eq!(b.case, WebShearCase::ShearYielding);
    assert!((a.phi_v - 1.00).abs() < 1e-12);
    assert!((b.phi_v - 0.90).abs() < 1e-12);
    assert!((a.cv - 1.0).abs() < 1e-12);
    assert!((b.cv - 1.0).abs() < 1e-12);
}

// ---------------------------------------------------------------------------
// Degenerate and invalid input
// ---------------------------------------------------------------------------

#[test]
fn zero_span_is_refused_everywhere_it_appears() {
    assert!(matches!(
        simply_supported_uniform(
            LineLoad::kn_per_m(10.0),
            Length::mm(0.0),
            STEEL_E,
            SecondMoment::cm4(23_700.0)
        ),
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
fn negative_area_is_refused_in_both_materials() {
    let mut steel = profiles::wf("WF 400x200x8x13").expect("profile");
    steel.area = Area::mm2(-8412.0);
    assert!(matches!(
        check_flexure(&FlexureInput::new(
            steel,
            FY,
            Length::m(2.0),
            Moment::kn_m(100.0)
        )),
        Err(SniError::NonPositive {
            parameter: "area",
            ..
        })
    ));

    let mut concrete = ConcreteBeamInput::new(
        Length::mm(300.0),
        Length::mm(500.0),
        Area::mm2(1500.0),
        Stress::mpa(25.0),
        Stress::mpa(400.0),
        Moment::kn_m(200.0),
    );
    concrete.steel_area = Area::mm2(-100.0);
    assert!(matches!(
        check_beam_flexure(&concrete),
        Err(SniError::NonPositive {
            parameter: "steel_area",
            ..
        })
    ));
}

#[test]
fn a_zero_demand_gives_a_zero_ratio_not_a_division_by_zero() {
    let check = check_flexure(&FlexureInput::new(
        profiles::wf("WF 400x200x8x13").expect("profile"),
        FY,
        Length::m(2.0),
        Moment::kn_m(0.0),
    ))
    .expect("check");
    assert!((check.ratio - 0.0).abs() < 1e-15);
    assert!(check.adequate);
}

#[test]
fn an_all_zero_load_set_produces_zero_in_every_combination() {
    let report = evaluate(&lrfd_combinations(), &LoadSet::zero()).expect("evaluate");
    assert_eq!(report.results.len(), 8);
    for result in &report.results {
        assert!((result.value.value() - 0.0).abs() < 1e-15);
    }
    // With nothing to separate them the first combination is reported.
    assert_eq!(report.governing_index, 0);
    assert_eq!(report.minimum_index, 0);
}

#[test]
fn a_non_finite_input_never_reaches_the_arithmetic() {
    let section = profiles::wf("WF 400x200x8x13").expect("profile");
    let err = check_flexure(&FlexureInput::new(
        section,
        Stress::mpa(f64::NAN),
        Length::m(2.0),
        Moment::kn_m(100.0),
    ))
    .expect_err("must refuse");
    assert!(matches!(err, SniError::NotFinite { parameter: "fy" }));
}

#[test]
fn error_messages_name_the_parameter_and_the_value() {
    let err = SniError::NonPositive {
        parameter: "effective_depth",
        value: -50.0,
    };
    let text = err.to_string();
    assert!(text.contains("effective_depth"));
    assert!(text.contains("-50"));
}

#[test]
fn a_load_effect_set_in_one_unit_stays_in_that_unit() {
    // Effects supplied as forces in newtons come back as newtons.
    let loads = LoadSet::zero()
        .with_dead(LoadEffect::from_force(Force::kn(100.0)))
        .with_live(LoadEffect::from_force(Force::kn(50.0)));
    let report = evaluate(&lrfd_combinations(), &loads).expect("evaluate");
    // 1.2*100 000 + 1.6*50 000 = 200 000 N
    let governing = report.governing().value.as_force();
    assert!((governing.to_n() - 200_000.0).abs() < 1e-9);
    assert!((governing.to_kn() - 200.0).abs() < 1e-12);
}
