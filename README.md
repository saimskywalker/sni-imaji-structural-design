# sni-struct

Indonesian structural engineering design checks in Rust: steel members to
SNI 1729, reinforced concrete to SNI 2847, LRFD load combinations to SNI 1727,
and seismic base shear to SNI 1726.

No dependencies beyond the standard library.


## Why this exists

Engineering calculation in Indonesia largely runs on spreadsheets passed
between offices by email. No version control, no tests, no audit trail, and no
way to tell whether the copy you were sent still matches the one that was
checked. A library with a test suite is a modest improvement, but a real one:
the arithmetic can be reviewed, the assumptions are written down, and a change
that breaks a result fails loudly.

For irrigation and hydraulic structures, see
[kp-irigasi](https://github.com/saimskywalker/kp-irigasi). Structural design and
hydraulic design are different enough in practice that they are kept apart.

## What it does

| Module              | Standard  | Covers |
|---------------------|-----------|--------|
| `steel::section`    | SNI 1729  | Rolled I geometry, `ry`, `rts`, `J`, `Cw`, slenderness classification (cl. B4) |
| `steel::profiles`   | catalogue | 20 common Indonesian WF and H profiles |
| `steel::flexure`    | SNI 1729  | `Mp`, `Lp`, `Lr`, the three LTB zones (cl. F2), flange local buckling (cl. F3) |
| `steel::shear`      | SNI 1729  | `Vn = 0.6*Fy*Aw*Cv` with all four web slenderness cases (cl. G2.1) |
| `loads`             | SNI 1727  | LRFD strength combinations (cl. 2.3.1), governing and reversal cases |
| `concrete::flexure` | SNI 2847  | Singly reinforced rectangular beam, `beta1`, strain-based `phi`, `As,min`, `As,max` |
| `deflection`        | SNI 2847  | Span over ratio limits, elastic deflection formulas |
| `seismic`           | SNI 1726  | Equivalent lateral force base shear `V = Cs*W` (cl. 7.8) |

## Unit convention

One convention, strict SI, everywhere. Every public signature takes and returns
a newtype, so kN cannot be silently mixed with N.

| Quantity        | Unit                    | Type              |
|-----------------|-------------------------|-------------------|
| Length          | millimetre, mm          | `Length`          |
| Area            | mm2                     | `Area`            |
| Second moment   | mm4                     | `SecondMoment`    |
| Section modulus | mm3                     | `SectionModulus`  |
| Force           | newton, N               | `Force`           |
| Line load       | N/mm, equal to kN/m     | `LineLoad`        |
| Moment          | newton millimetre, N.mm | `Moment`          |
| Stress, modulus | megapascal, MPa         | `Stress`          |

Conversion happens only in named constructors (`Force::kn`, `Moment::kn_m`,
`Length::m`, `Area::cm2`, `SecondMoment::cm4`) and the matching accessors
(`to_kn`, `to_kn_m`, `to_m`). Silent unit mixing, the classic failure mode in
design software, is a compile error rather than a wrong answer.

Load combinations are the one exception, because they are linear and apply
equally to forces, moments and reactions. They use `LoadEffect`, which carries
the caller's promise that every effect in one `LoadSet` shares a unit;
`LoadEffect::from_force` and `LoadEffect::from_moment` pin it to N or N.mm.

## Usage

```rust
use sni_struct::loads::{evaluate, lrfd_combinations, LoadSet};
use sni_struct::steel::{check_flexure, profiles, FlexureInput, LimitState};
use sni_struct::units::{Length, LoadEffect, Moment, Stress};
use sni_struct::SniError;

fn design_a_floor_beam() -> Result<(), SniError> {
    // Unfactored midspan moments, kN.m.
    let loads = LoadSet::zero()
        .with_dead(LoadEffect::new(36.0))
        .with_live(LoadEffect::new(54.0));
    let report = evaluate(&lrfd_combinations(), &loads)?;

    // 1.2*36 + 1.6*54 = 129.6 kN.m
    let mu = Moment::kn_m(report.governing().value.value());

    let section = profiles::wf("WF 350x175x7x11").expect("bundled profile");
    let check = check_flexure(&FlexureInput::new(
        section,
        Stress::mpa(250.0), // BJ 41
        Length::m(3.0),     // compression flange braced at midspan
        mu,
    ))?;

    // Lp = 1965 mm, Lr = 5779 mm, so Lb = 3000 mm is in the inelastic zone.
    assert_eq!(check.governing, LimitState::LateralTorsionalBuckling);
    assert!(check.adequate);

    // phi*Mn = 170.97 kN.m, ratio = 0.758
    println!(
        "phi*Mn = {:.2} kN.m, ratio = {:.3}",
        check.phi_mn.to_kn_m(),
        check.ratio
    );
    Ok(())
}
```

Two runnable examples print full calculation reports:

```
cargo run -p sni-struct --example steel_beam_check
cargo run -p sni-struct --example concrete_beam_check
```

## Design principles

**No panics in library code.** Input that is not physically meaningful, or that
falls outside the implemented clauses, returns `Err(SniError)` naming the
parameter. `unwrap` and `expect` appear only in tests and examples.

**No bare booleans.** Every check returns a struct with the demand, the capacity,
the demand over capacity ratio, the governing limit state, and the intermediate
values needed to reproduce the calculation by hand: `Lp`, `Lr`, `Cv`, `beta1`,
the strain at nominal strength, the slenderness ratios and their limits.

**Refuse rather than guess.** A section with a non-compact web returns
`SniError::UnsupportedSection` rather than an answer from a clause that does not
apply to it.

## Relationship to the SNI documents

SNI standards are copyrighted by BSN and sold commercially. This crate implements
the equations and procedures they set out, which are methods and facts rather
than copyrightable expression, and cites clause numbers so a result can be traced
back. It reproduces no text and no table.

Where a standard has a lookup table, the value is an input parameter documented
with the clause it comes from. The site coefficients and the design spectral
accelerations `Sds` and `Sd1`, the response modification coefficient `R`, the
importance factor `Ie`, the period coefficients `Ct`, `x` and `Cu`, and the web
stiffener coefficient `kv` are all supplied by the caller.

The steel profile catalogue is manufacturer catalogue data for the JIS G3192 /
SNI 07-0329 hot-rolled series, not standard text. Its `Zx` values are computed
from the plate geometry, ignoring root fillets, which is slightly conservative.

You still need the standards. This crate is not a substitute for them.

## Scope and known limitations

- Steel flexure covers doubly symmetric I sections bending about the strong axis
  with a compact web. Clauses F4 and F5 (non-compact and slender webs) are not
  implemented and are refused rather than approximated.
- Steel shear covers clause G2.1 only. Tension field action (cl. G2.2) is not
  implemented, so a stiffened girder that could mobilise it gets a conservative
  answer.
- Concrete flexure covers singly reinforced rectangular sections. Compression
  reinforcement, flanged sections and prestressing are out of scope.
- Seismic covers the base shear of the equivalent lateral force procedure only.
  Vertical distribution, torsion, drift and modal analysis are out of scope.
- `Cb` is an input, not derived from a moment diagram. The default of 1.0 is
  always conservative.

## Disclaimer

This library is a calculation aid. It is not a substitute for a licensed
engineer's judgement, and no result from it should be used in a real design
without being checked against the governing edition of the relevant SNI
standard. The author accepts no liability for the use of these calculations.

## Licence

MIT OR Apache-2.0, matching the workspace.
