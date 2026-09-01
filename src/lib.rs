//! Indonesian structural engineering design checks: SNI 1729 (steel),
//! SNI 2847 (reinforced concrete), SNI 1727 (load combinations) and
//! SNI 1726 (seismic base shear).
//!
//! # Unit convention
//!
//! One convention, strict SI, everywhere:
//!
//! | Quantity        | Unit                    | Type                          |
//! |-----------------|-------------------------|-------------------------------|
//! | Length          | millimetre, mm          | [`units::Length`]             |
//! | Area            | mm2                     | [`units::Area`]               |
//! | Second moment   | mm4                     | [`units::SecondMoment`]       |
//! | Section modulus | mm3                     | [`units::SectionModulus`]     |
//! | Force           | newton, N               | [`units::Force`]              |
//! | Line load       | N/mm, equal to kN/m     | [`units::LineLoad`]           |
//! | Moment          | newton millimetre, N·mm | [`units::Moment`]             |
//! | Stress, modulus | megapascal, MPa         | [`units::Stress`]             |
//!
//! Every public signature takes and returns these newtypes, so kN cannot be
//! silently mixed with N. Conversion happens only in named constructors such as
//! `Force::kn`, `Moment::kn_m`, `Length::m`, and in the matching accessors
//! `to_kn`, `to_kn_m`, `to_m`. See [`units`] for the full list.
//!
//! # Error handling
//!
//! Library code never panics. Input that is not physically meaningful, or that
//! falls outside the clauses implemented here, comes back as [`SniError`].
//! `unwrap` and `expect` appear only in tests, examples and doc examples.
//!
//! # What every check returns
//!
//! No check returns a bare boolean. Each returns a struct carrying the demand,
//! the capacity, the demand over capacity ratio, the governing limit state, and
//! the intermediate values needed to reproduce the calculation by hand: `Lp`,
//! `Lr`, `Cv`, `beta1`, the strain at nominal strength, the slenderness ratios
//! and their limits, and so on.
//!
//! # Relationship to the SNI documents
//!
//! SNI standards are copyrighted by BSN and sold commercially. This crate
//! implements the **equations and procedures** they set out, which are methods and
//! facts rather than copyrightable expression, and cites clause numbers so a
//! result can be traced back. It reproduces no text, and no table. Where a
//! standard has a lookup table, the value is an input parameter documented with
//! the clause it comes from: site coefficients and `Sds`, `Sd1`, the response
//! modification coefficient `R`, the importance factor `Ie`, the period
//! coefficients `Ct`, `x` and `Cu`, and the stiffener coefficient `kv` are all
//! supplied by the caller. The steel profile catalogue in
//! [`steel::profiles`] is manufacturer catalogue data, not standard text.
//!
//! You still need the standards. This crate is not a substitute for them.
//!
//! # Design responsibility
//!
//! This library is a calculation aid. It is not a substitute for a licensed
//! engineer's judgement, and no result from it should be used in a real design
//! without being checked against the governing edition of the relevant SNI
//! standard. The author accepts no liability for the use of these calculations.
//!
//! # Example
//!
//! A steel beam check from end to end.
//!
//! ```
//! use sni_struct::loads::{evaluate, lrfd_combinations, LoadSet};
//! use sni_struct::steel::{check_flexure, check_shear, profiles, FlexureInput, ShearInput};
//! use sni_struct::units::{Force, Length, LoadEffect, Moment, Stress};
//!
//! // Unfactored midspan moments, kN.m.
//! let loads = LoadSet::zero()
//!     .with_dead(LoadEffect::new(60.0))
//!     .with_live(LoadEffect::new(45.0));
//! let report = evaluate(&lrfd_combinations(), &loads)?;
//! let mu = Moment::kn_m(report.governing().value.value());
//!
//! let section = profiles::wf("WF 400x200x8x13").expect("bundled profile");
//! let flexure = check_flexure(&FlexureInput::new(
//!     section.clone(),
//!     Stress::mpa(240.0),
//!     Length::m(2.0),
//!     mu,
//! ))?;
//! let shear = check_shear(&ShearInput::new(section, Stress::mpa(240.0), Force::kn(150.0)))?;
//!
//! assert!(flexure.adequate);
//! assert!(shear.adequate);
//! # Ok::<(), sni_struct::SniError>(())
//! ```

#![deny(missing_docs)]
#![deny(unsafe_code)]
#![doc(test(attr(deny(warnings))))]

pub mod concrete;
pub mod deflection;
pub mod error;
pub mod loads;
pub mod seismic;
pub mod steel;
pub mod units;

pub use error::{Result, SniError};
