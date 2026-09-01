//! A complete SNI 2847 flexural check of a singly reinforced rectangular beam,
//! printed as a calculation report.
//!
//! Run with:
//!
//! ```text
//! cargo run -p sni-struct --example concrete_beam_check
//! ```

use std::error::Error;

use sni_struct::concrete::{check_beam_flexure, ConcreteBeamInput, Confinement};
use sni_struct::loads::{evaluate, lrfd_combinations, LoadSet};
use sni_struct::units::{Area, Length, LoadEffect, Moment, Stress};

fn main() -> Result<(), Box<dyn Error>> {
    // ---- Design data -----------------------------------------------------
    let width = Length::mm(350.0);
    let overall_depth = Length::mm(620.0);
    let effective_depth = Length::mm(550.0);
    let fc = Stress::mpa(30.0);
    let fy = Stress::mpa(420.0);

    // 4 D25: area of one D25 bar is pi*25^2/4 = 490.87 mm2.
    let bar_count = 4.0;
    let bar_diameter = 25.0_f64;
    let bar_area = std::f64::consts::PI * bar_diameter * bar_diameter / 4.0;
    let steel_area = Area::mm2(bar_count * bar_area);

    println!("SNI 2847 reinforced concrete beam check");
    println!("=======================================");
    println!();
    println!("Units: mm, mm2, MPa, N.mm. Moments are printed in kN.m for reading.");
    println!();
    println!(
        "Section     b = {:.0} mm, h = {:.0} mm, d = {:.0} mm",
        width.to_mm(),
        overall_depth.to_mm(),
        effective_depth.to_mm()
    );
    println!(
        "Materials   fc' = {:.0} MPa, fy = {:.0} MPa",
        fc.to_mpa(),
        fy.to_mpa()
    );
    println!(
        "Reinf.      {bar_count:.0} D{bar_diameter:.0} = {:.1} mm2, tied stirrups",
        steel_area.to_mm2()
    );
    println!();

    // ---- Load combinations, SNI 1727 --------------------------------------
    // Unfactored midspan moments from the frame analysis, kN.m.
    let moments = LoadSet::zero()
        .with_dead(LoadEffect::new(120.0))
        .with_live(LoadEffect::new(95.0))
        .with_seismic(LoadEffect::new(70.0));

    let report = evaluate(&lrfd_combinations(), &moments)?;

    println!("1. Load combinations (SNI 1727 cl. 2.3.1), midspan moment in kN.m");
    println!("   {:<36}{:>12}", "combination", "Mu");
    println!("   {}", "-".repeat(48));
    for result in &report.results {
        println!(
            "   {:<36}{:>12.2}",
            result.combination.name,
            result.value.value()
        );
    }
    println!(
        "   governing: {} at {:.2} kN.m",
        report.governing().combination.name,
        report.governing().value.value()
    );
    println!();

    let mu = Moment::kn_m(report.governing().value.value());

    // ---- Flexure, SNI 2847 -------------------------------------------------
    let input = ConcreteBeamInput::new(width, effective_depth, steel_area, fc, fy, mu)
        .with_confinement(Confinement::Tied);
    let check = check_beam_flexure(&input)?;

    println!("2. Equivalent rectangular stress block (SNI 2847 cl. 22.2.2)");
    println!("   beta1   = {:>12.4}          (from fc')", check.beta1);
    println!(
        "   a       = {:>12.3} mm     As*fy/(0.85*fc'*b)",
        check.a.to_mm()
    );
    println!("   c       = {:>12.3} mm     a/beta1", check.c.to_mm());
    println!("   rho     = {:>12.5}          As/(b*d)", check.rho);
    println!();

    println!("3. Strain check and phi (SNI 2847 cl. 21.2.2)");
    println!("   eps_ty  = {:>12.5}          fy/Es", check.epsilon_ty);
    println!(
        "   eps_t   = {:>12.5}          0.003*(d - c)/c",
        check.epsilon_t
    );
    println!("   section is {:?}", check.behaviour);
    println!("   phi     = {:>12.3}", check.phi);
    println!();

    println!("4. Flexural strength (SNI 2847 cl. 22.2)");
    println!(
        "   Mn      = {:>12.2} kN.m   As*fy*(d - a/2)",
        check.mn.to_kn_m()
    );
    println!("   phi*Mn  = {:>12.2} kN.m", check.phi_mn.to_kn_m());
    println!("   Mu      = {:>12.2} kN.m", check.mu.to_kn_m());
    println!(
        "   Mu/phiMn= {:>12.3}          {}",
        check.ratio,
        verdict(check.ratio <= 1.0)
    );
    println!();

    println!("5. Reinforcement limits");
    println!(
        "   As,min  = {:>12.1} mm2    cl. 9.6.1.2, max(0.25*sqrt(fc')/fy, 1.4/fy)*b*d   {}",
        check.as_min.to_mm2(),
        verdict(check.min_reinforcement_ok)
    );
    println!("   As      = {:>12.1} mm2", input.steel_area.to_mm2());
    println!(
        "   As,max  = {:>12.1} mm2    cl. 9.3.3.1, the area at eps_t = 0.004        {}",
        check.as_max.to_mm2(),
        verdict(check.max_reinforcement_ok)
    );
    println!();

    println!("Governing limit state: {}", check.governing);
    println!(
        "Result: {}",
        if check.adequate {
            "section is adequate"
        } else {
            "section is NOT adequate"
        }
    );
    println!();
    println!("This report is a calculation aid. It is not a substitute for a licensed");
    println!("engineer's judgement. Check every value against SNI 2847 and SNI 1727");
    println!("before using it in a real design.");

    Ok(())
}

fn verdict(ok: bool) -> &'static str {
    if ok {
        "OK"
    } else {
        "NOT OK"
    }
}
