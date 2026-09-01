//! A complete SNI 1729 check of a simply supported steel floor beam, printed as
//! a calculation report.
//!
//! Run with:
//!
//! ```text
//! cargo run -p sni-struct --example steel_beam_check
//! ```

use std::error::Error;

use sni_struct::deflection::{check_deflection, simply_supported_uniform, DeflectionLimit};
use sni_struct::loads::{evaluate, lrfd_combinations, LoadSet};
use sni_struct::steel::{check_flexure, check_shear, profiles, FlexureInput, ShearInput, STEEL_E};
use sni_struct::units::{Force, Length, LineLoad, LoadEffect, Moment, Stress};

fn main() -> Result<(), Box<dyn Error>> {
    // ---- Design data -----------------------------------------------------
    // Units are stated at every call site by the constructor used.
    let designation = "WF 350x175x7x11";
    let fy = Stress::mpa(250.0); // BJ 41
    let span = Length::m(6.0);
    let unbraced = Length::m(3.0); // lateral bracing at midspan
    let dead = LineLoad::kn_per_m(8.0);
    let live = LineLoad::kn_per_m(12.0);

    let section = profiles::wf(designation).ok_or("profile not in the bundled catalogue")?;

    println!("SNI 1729 steel beam check");
    println!("=========================");
    println!();
    println!("Units: mm, N, MPa, N.mm. Values are printed in kN and kN.m for reading.");
    println!();
    println!("Beam        simply supported, span {:.2} m", span.to_m());
    println!("Section     {designation}");
    println!(
        "Steel       fy = {:.0} MPa, E = {:.0} MPa",
        fy.to_mpa(),
        STEEL_E.to_mpa()
    );
    println!(
        "Bracing     compression flange braced at midspan, Lb = {:.2} m",
        unbraced.to_m()
    );
    println!(
        "Loads       dead {:.1} kN/m, live {:.1} kN/m (service)",
        dead.to_kn_per_m(),
        live.to_kn_per_m()
    );
    println!();

    // ---- Load combinations, SNI 1727 --------------------------------------
    // Simple span: M = w*L^2/8 and V = w*L/2, so the combinations can be applied
    // directly to the moments and shears.
    let l = span.to_m();
    let moment_of = |w: LineLoad| LoadEffect::new(w.to_kn_per_m() * l * l / 8.0);
    let shear_of = |w: LineLoad| LoadEffect::new(w.to_kn_per_m() * l / 2.0);

    let moments = LoadSet::zero()
        .with_dead(moment_of(dead))
        .with_live(moment_of(live));
    let shears = LoadSet::zero()
        .with_dead(shear_of(dead))
        .with_live(shear_of(live));

    let moment_report = evaluate(&lrfd_combinations(), &moments)?;
    let shear_report = evaluate(&lrfd_combinations(), &shears)?;

    println!("1. Load combinations (SNI 1727 cl. 2.3.1), midspan moment in kN.m");
    println!("   {:<36}{:>12}", "combination", "Mu");
    println!("   {}", "-".repeat(48));
    for result in &moment_report.results {
        println!(
            "   {:<36}{:>12.2}",
            result.combination.name,
            result.value.value()
        );
    }
    println!(
        "   governing: {} at {:.2} kN.m",
        moment_report.governing().combination.name,
        moment_report.governing().value.value()
    );
    println!();

    let mu = Moment::kn_m(moment_report.governing().value.value());
    let vu = Force::kn(shear_report.governing().value.value());

    // ---- Flexure, SNI 1729 chapter F --------------------------------------
    let flexure = check_flexure(&FlexureInput::new(section.clone(), fy, unbraced, mu))?;

    println!("2. Section classification (SNI 1729 cl. B4, Table B4.1b)");
    let c = &flexure.classification;
    println!(
        "   flange  bf/2tf = {:>7.3}   lambda_p = {:>7.3}   lambda_r = {:>7.3}   {:?}",
        c.lambda_flange, c.lambda_p_flange, c.lambda_r_flange, c.flange
    );
    println!(
        "   web     h/tw   = {:>7.3}   lambda_p = {:>7.3}   lambda_r = {:>7.3}   {:?}",
        c.lambda_web, c.lambda_p_web, c.lambda_r_web, c.web
    );
    println!("   section: {:?}", c.overall);
    println!();

    println!("3. Flexural strength (SNI 1729 cl. F2)");
    println!("   Zx      = {:>12.0} mm3", section.zx.to_mm3());
    println!("   Sx      = {:>12.0} mm3", section.sx.to_mm3());
    println!("   ry      = {:>12.2} mm", flexure.ry.to_mm());
    println!("   rts     = {:>12.2} mm", flexure.rts.to_mm());
    println!("   J       = {:>12.0} mm4", flexure.j);
    println!("   ho      = {:>12.2} mm", flexure.ho.to_mm());
    println!("   Mp      = {:>12.2} kN.m   (Zx*fy)", flexure.mp.to_kn_m());
    println!("   My      = {:>12.2} kN.m   (Sx*fy)", flexure.my.to_kn_m());
    println!("   Lp      = {:>12.1} mm", flexure.lp.to_mm());
    println!("   Lr      = {:>12.1} mm", flexure.lr.to_mm());
    println!(
        "   Lb      = {:>12.1} mm     -> {:?} zone",
        flexure.lb.to_mm(),
        flexure.ltb_zone
    );
    println!("   Cb      = {:>12.2}", flexure.cb);
    println!("   Mn,LTB  = {:>12.2} kN.m", flexure.mn_ltb.to_kn_m());
    match flexure.mn_flb {
        Some(value) => println!("   Mn,FLB  = {:>12.2} kN.m", value.to_kn_m()),
        None => println!("   Mn,FLB  =          n/a   (compact flange)"),
    }
    println!(
        "   Mn      = {:>12.2} kN.m   governed by {}",
        flexure.mn.to_kn_m(),
        flexure.governing
    );
    println!("   phi_b   = {:>12.2}", flexure.phi_b);
    println!("   phi*Mn  = {:>12.2} kN.m", flexure.phi_mn.to_kn_m());
    println!("   Mu      = {:>12.2} kN.m", flexure.mu.to_kn_m());
    println!(
        "   Mu/phiMn= {:>12.3}          {}",
        flexure.ratio,
        verdict(flexure.adequate)
    );
    println!();

    // ---- Shear, SNI 1729 chapter G ----------------------------------------
    let shear = check_shear(&ShearInput::new(section.clone(), fy, vu))?;

    println!("4. Shear strength (SNI 1729 cl. G2.1)");
    println!("   h/tw    = {:>12.3}", shear.h_over_tw);
    println!("   2.24*sqrt(E/fy) = {:>4.3}", shear.stocky_limit);
    println!("   kv      = {:>12.2}", shear.kv);
    println!(
        "   Cv      = {:>12.3}          case {:?}",
        shear.cv, shear.case
    );
    println!("   Aw      = {:>12.0} mm2    (d*tw)", shear.aw.to_mm2());
    println!(
        "   Vn      = {:>12.2} kN     (0.6*fy*Aw*Cv)",
        shear.vn.to_kn()
    );
    println!("   phi_v   = {:>12.2}", shear.phi_v);
    println!("   phi*Vn  = {:>12.2} kN", shear.phi_vn.to_kn());
    println!("   Vu      = {:>12.2} kN", shear.vu.to_kn());
    println!(
        "   Vu/phiVn= {:>12.3}          {}",
        shear.ratio,
        verdict(shear.adequate)
    );
    println!();

    // ---- Serviceability ---------------------------------------------------
    let delta_live = simply_supported_uniform(live, span, STEEL_E, section.ix)?;
    let deflection = check_deflection(delta_live, span, DeflectionLimit::Floor)?;

    println!("5. Live load deflection (SNI 2847 cl. 24.2.2 style limit, L/360)");
    println!("   Ix      = {:>12.0} mm4", section.ix.to_mm4());
    println!(
        "   delta   = {:>12.2} mm     (5*w*L^4/384EI)",
        deflection.actual.to_mm()
    );
    println!(
        "   allowed = {:>12.2} mm     ({})",
        deflection.allowable.to_mm(),
        deflection.limit
    );
    println!(
        "   ratio   = {:>12.3}          {}",
        deflection.ratio,
        verdict(deflection.adequate)
    );
    println!();

    let all_ok = flexure.adequate && shear.adequate && deflection.adequate;
    println!(
        "Result: {}",
        if all_ok {
            "section is adequate"
        } else {
            "section is NOT adequate"
        }
    );
    println!();
    println!("This report is a calculation aid. It is not a substitute for a licensed");
    println!("engineer's judgement. Check every value against SNI 1729 and SNI 1727");
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
