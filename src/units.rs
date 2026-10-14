//! Unit newtypes for the strict SI convention used across this crate.
//!
//! # Unit convention
//!
//! Every quantity in `imaji-structural-design` is stored in one canonical unit and nothing else:
//!
//! | Quantity        | Type             | Canonical unit          |
//! |-----------------|------------------|-------------------------|
//! | Length          | [`Length`]       | millimetre, mm          |
//! | Area            | [`Area`]         | square millimetre, mm2  |
//! | Second moment   | [`SecondMoment`] | mm4                     |
//! | Section modulus | [`SectionModulus`] | mm3                   |
//! | Force           | [`Force`]        | newton, N               |
//! | Line load       | [`LineLoad`]     | newton per mm, N/mm     |
//! | Moment          | [`Moment`]       | newton millimetre, N·mm |
//! | Stress          | [`Stress`]       | megapascal, MPa = N/mm2 |
//!
//! The types are distinct, so a length cannot be passed where a force is wanted, and
//! there is exactly one internal representation per quantity. Conversion happens only
//! in the named constructors (`Force::kn`, `Length::m`, ...) and the named accessors
//! (`Force::to_kn`, `Length::to_m`, ...). Silent mixing of kN and N, the classic
//! failure mode in design software, is therefore a compile error rather than a
//! wrong answer.
//!
//! Note the convenient identities of this system: 1 kN/m equals exactly 1 N/mm, and
//! 1 MPa equals exactly 1 N/mm2.
//!
//! Constructors never panic and never validate. Validation happens at the design
//! check entry points, which return [`crate::SniError`] for non-finite, zero or
//! negative input where those are not physically meaningful.

use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

macro_rules! unit_newtype {
    ($name:ident, $canonical:expr) => {
        impl $name {
            /// The stored value in this type's canonical unit.
            ///
            /// Prefer the named accessors where they exist; this is the escape hatch
            /// for arithmetic the typed operators do not cover.
            #[inline]
            pub const fn raw(self) -> f64 {
                self.0
            }

            /// True when the value is finite and strictly greater than zero.
            #[inline]
            pub fn is_positive_finite(self) -> bool {
                self.0.is_finite() && self.0 > 0.0
            }

            /// True when the value is finite and not negative.
            #[inline]
            pub fn is_non_negative_finite(self) -> bool {
                self.0.is_finite() && self.0 >= 0.0
            }

            /// The larger of two values.
            #[inline]
            pub fn max(self, other: Self) -> Self {
                Self(self.0.max(other.0))
            }

            /// The smaller of two values.
            #[inline]
            pub fn min(self, other: Self) -> Self {
                Self(self.0.min(other.0))
            }

            /// The magnitude, discarding sign.
            #[inline]
            pub fn abs(self) -> Self {
                Self(self.0.abs())
            }
        }

        impl Add for $name {
            type Output = Self;
            #[inline]
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }

        impl Mul<f64> for $name {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: f64) -> Self {
                Self(self.0 * rhs)
            }
        }

        impl Mul<$name> for f64 {
            type Output = $name;
            #[inline]
            fn mul(self, rhs: $name) -> $name {
                $name(self * rhs.0)
            }
        }

        impl Div<f64> for $name {
            type Output = Self;
            #[inline]
            fn div(self, rhs: f64) -> Self {
                Self(self.0 / rhs)
            }
        }

        impl Div for $name {
            type Output = f64;
            /// Dividing like by like gives a dimensionless ratio.
            #[inline]
            fn div(self, rhs: Self) -> f64 {
                self.0 / rhs.0
            }
        }

        impl Neg for $name {
            type Output = Self;
            #[inline]
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let canonical: &str = $canonical;
                if canonical.is_empty() {
                    write!(f, "{:.4}", self.0)
                } else {
                    write!(f, "{:.4} {canonical}", self.0)
                }
            }
        }
    };
}

/// A length, stored in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Length(f64);
unit_newtype!(Length, "mm");

impl Length {
    /// Construct from millimetres.
    #[inline]
    pub const fn mm(value: f64) -> Self {
        Self(value)
    }

    /// Construct from centimetres.
    #[inline]
    pub const fn cm(value: f64) -> Self {
        Self(value * 10.0)
    }

    /// Construct from metres.
    #[inline]
    pub const fn m(value: f64) -> Self {
        Self(value * 1000.0)
    }

    /// Value in millimetres.
    #[inline]
    pub const fn to_mm(self) -> f64 {
        self.0
    }

    /// Value in metres.
    #[inline]
    pub const fn to_m(self) -> f64 {
        self.0 / 1000.0
    }
}

/// An area, stored in square millimetres.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Area(f64);
unit_newtype!(Area, "mm2");

impl Area {
    /// Construct from square millimetres.
    #[inline]
    pub const fn mm2(value: f64) -> Self {
        Self(value)
    }

    /// Construct from square centimetres, the unit most steel catalogues use.
    #[inline]
    pub const fn cm2(value: f64) -> Self {
        Self(value * 100.0)
    }

    /// Value in square millimetres.
    #[inline]
    pub const fn to_mm2(self) -> f64 {
        self.0
    }

    /// Value in square centimetres.
    #[inline]
    pub const fn to_cm2(self) -> f64 {
        self.0 / 100.0
    }
}

/// A second moment of area, stored in mm4.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct SecondMoment(f64);
unit_newtype!(SecondMoment, "mm4");

impl SecondMoment {
    /// Construct from mm4.
    #[inline]
    pub const fn mm4(value: f64) -> Self {
        Self(value)
    }

    /// Construct from cm4, the unit most steel catalogues use.
    #[inline]
    pub const fn cm4(value: f64) -> Self {
        Self(value * 10_000.0)
    }

    /// Value in mm4.
    #[inline]
    pub const fn to_mm4(self) -> f64 {
        self.0
    }

    /// Value in cm4.
    #[inline]
    pub const fn to_cm4(self) -> f64 {
        self.0 / 10_000.0
    }
}

/// A section modulus, elastic or plastic, stored in mm3.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct SectionModulus(f64);
unit_newtype!(SectionModulus, "mm3");

impl SectionModulus {
    /// Construct from mm3.
    #[inline]
    pub const fn mm3(value: f64) -> Self {
        Self(value)
    }

    /// Construct from cm3, the unit most steel catalogues use.
    #[inline]
    pub const fn cm3(value: f64) -> Self {
        Self(value * 1000.0)
    }

    /// Value in mm3.
    #[inline]
    pub const fn to_mm3(self) -> f64 {
        self.0
    }

    /// Value in cm3.
    #[inline]
    pub const fn to_cm3(self) -> f64 {
        self.0 / 1000.0
    }
}

/// A force, stored in newtons.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Force(f64);
unit_newtype!(Force, "N");

impl Force {
    /// Construct from newtons.
    #[inline]
    pub const fn n(value: f64) -> Self {
        Self(value)
    }

    /// Construct from kilonewtons.
    #[inline]
    pub const fn kn(value: f64) -> Self {
        Self(value * 1000.0)
    }

    /// Value in newtons.
    #[inline]
    pub const fn to_n(self) -> f64 {
        self.0
    }

    /// Value in kilonewtons.
    #[inline]
    pub const fn to_kn(self) -> f64 {
        self.0 / 1000.0
    }
}

/// A uniformly distributed line load, stored in newtons per millimetre.
///
/// 1 kN/m is exactly 1 N/mm, so [`LineLoad::kn_per_m`] and [`LineLoad::n_per_mm`]
/// take the same number.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct LineLoad(f64);
unit_newtype!(LineLoad, "N/mm");

impl LineLoad {
    /// Construct from newtons per millimetre.
    #[inline]
    pub const fn n_per_mm(value: f64) -> Self {
        Self(value)
    }

    /// Construct from kilonewtons per metre, numerically identical to N/mm.
    #[inline]
    pub const fn kn_per_m(value: f64) -> Self {
        Self(value)
    }

    /// Value in newtons per millimetre.
    #[inline]
    pub const fn to_n_per_mm(self) -> f64 {
        self.0
    }

    /// Value in kilonewtons per metre.
    #[inline]
    pub const fn to_kn_per_m(self) -> f64 {
        self.0
    }
}

/// A bending moment, stored in newton millimetres.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Moment(f64);
unit_newtype!(Moment, "N.mm");

impl Moment {
    /// Construct from newton millimetres.
    #[inline]
    pub const fn n_mm(value: f64) -> Self {
        Self(value)
    }

    /// Construct from kilonewton metres, the unit engineers quote moments in.
    #[inline]
    pub const fn kn_m(value: f64) -> Self {
        Self(value * 1.0e6)
    }

    /// Value in newton millimetres.
    #[inline]
    pub const fn to_n_mm(self) -> f64 {
        self.0
    }

    /// Value in kilonewton metres.
    #[inline]
    pub const fn to_kn_m(self) -> f64 {
        self.0 / 1.0e6
    }
}

/// A stress or elastic modulus, stored in megapascals (N/mm2).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Stress(f64);
unit_newtype!(Stress, "MPa");

impl Stress {
    /// Construct from megapascals.
    #[inline]
    pub const fn mpa(value: f64) -> Self {
        Self(value)
    }

    /// Construct from gigapascals.
    #[inline]
    pub const fn gpa(value: f64) -> Self {
        Self(value * 1000.0)
    }

    /// Value in megapascals.
    #[inline]
    pub const fn to_mpa(self) -> f64 {
        self.0
    }

    /// Value in gigapascals.
    #[inline]
    pub const fn to_gpa(self) -> f64 {
        self.0 / 1000.0
    }
}

/// A load effect of caller-chosen kind, used by the load combination module.
///
/// Load combinations are linear in the load effects, so the same coefficients apply
/// whether the effects are axial forces, shears, moments or reactions. This type
/// therefore carries no fixed unit: it carries the caller's promise that every
/// effect in one [`crate::loads::LoadSet`] is expressed in the *same* unit. Use
/// [`LoadEffect::from_force`] or [`LoadEffect::from_moment`] when the effects come
/// from typed quantities, which pins the unit to N or N·mm respectively.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct LoadEffect(f64);

impl LoadEffect {
    /// Construct from a bare number in the caller's chosen consistent unit.
    #[inline]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// Construct from a force. The resulting effect is in newtons.
    #[inline]
    pub const fn from_force(force: Force) -> Self {
        Self(force.to_n())
    }

    /// Construct from a moment. The resulting effect is in newton millimetres.
    #[inline]
    pub const fn from_moment(moment: Moment) -> Self {
        Self(moment.to_n_mm())
    }

    /// The bare numeric value, in whichever unit the caller supplied.
    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// Reinterpret as a force. Only valid if the effects were supplied in newtons.
    #[inline]
    pub const fn as_force(self) -> Force {
        Force::n(self.0)
    }

    /// Reinterpret as a moment. Only valid if the effects were supplied in N·mm.
    #[inline]
    pub const fn as_moment(self) -> Moment {
        Moment::n_mm(self.0)
    }

    /// True when the value is finite.
    #[inline]
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }
}

unit_newtype!(LoadEffect, "");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_conversions_round_trip() {
        assert!((Length::m(6.0).to_mm() - 6000.0).abs() < 1e-9);
        assert!((Length::cm(45.0).to_mm() - 450.0).abs() < 1e-9);
        assert!((Length::mm(2500.0).to_m() - 2.5).abs() < 1e-12);
    }

    #[test]
    fn force_and_moment_conversions() {
        // 250 kN = 250 000 N.
        assert!((Force::kn(250.0).to_n() - 250_000.0).abs() < 1e-9);
        // 308.63 kN.m = 308.63e6 N.mm
        assert!((Moment::kn_m(308.63).to_n_mm() - 308.63e6).abs() < 1e-3);
        assert!((Moment::n_mm(1.0e6).to_kn_m() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn catalogue_units_convert() {
        // WF 400x200 catalogue: A = 84.12 cm2 = 8412 mm2, Ix = 23700 cm4 = 2.37e8 mm4.
        assert!((Area::cm2(84.12).to_mm2() - 8412.0).abs() < 1e-9);
        assert!((SecondMoment::cm4(23_700.0).to_mm4() - 2.37e8).abs() < 1e-3);
        assert!((SectionModulus::cm3(1190.0).to_mm3() - 1_190_000.0).abs() < 1e-6);
    }

    #[test]
    fn line_load_kn_per_m_equals_n_per_mm() {
        assert_eq!(LineLoad::kn_per_m(12.5), LineLoad::n_per_mm(12.5));
    }

    #[test]
    fn like_over_like_is_dimensionless() {
        let ratio = Moment::kn_m(200.0) / Moment::kn_m(250.0);
        assert!((ratio - 0.8).abs() < 1e-12);
    }

    #[test]
    fn positivity_predicates() {
        assert!(Length::mm(1.0).is_positive_finite());
        assert!(!Length::mm(0.0).is_positive_finite());
        assert!(Length::mm(0.0).is_non_negative_finite());
        assert!(!Length::mm(-1.0).is_non_negative_finite());
        assert!(!Length::mm(f64::NAN).is_non_negative_finite());
        assert!(!Length::mm(f64::INFINITY).is_positive_finite());
    }
}
