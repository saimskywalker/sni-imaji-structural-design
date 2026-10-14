//! Error type for every fallible design check in this crate.
//!
//! Library code in `imaji-structural-design` never panics. Anything that cannot be computed,
//! because the input is not physically meaningful or falls outside the range the
//! implemented clauses cover, comes back as [`SniError`].

use core::fmt;

/// Convenience alias for results produced by this crate.
pub type Result<T> = core::result::Result<T, SniError>;

/// Everything that can go wrong in a design check.
#[derive(Debug, Clone, PartialEq)]
pub enum SniError {
    /// A parameter that must be strictly greater than zero was zero or negative.
    NonPositive {
        /// Name of the offending parameter, as it appears in the public signature.
        parameter: &'static str,
        /// The value supplied, in the parameter's canonical unit.
        value: f64,
    },
    /// A parameter that must not be negative was negative.
    Negative {
        /// Name of the offending parameter.
        parameter: &'static str,
        /// The value supplied, in the parameter's canonical unit.
        value: f64,
    },
    /// A parameter was NaN or infinite.
    NotFinite {
        /// Name of the offending parameter.
        parameter: &'static str,
    },
    /// A parameter was finite but outside the range the clause is written for.
    OutOfRange {
        /// Name of the offending parameter.
        parameter: &'static str,
        /// The value supplied.
        value: f64,
        /// Inclusive lower bound.
        min: f64,
        /// Inclusive upper bound.
        max: f64,
    },
    /// The supplied dimensions do not describe a physically buildable section,
    /// for example flanges thicker than half the overall depth.
    InconsistentGeometry {
        /// What is wrong, in one short phrase.
        detail: &'static str,
    },
    /// The section falls outside the clauses this crate implements.
    ///
    /// The reason names the clause family that would be needed instead, so the
    /// caller knows what to do by hand.
    UnsupportedSection {
        /// Why the section cannot be handled here.
        reason: &'static str,
    },
    /// A load combination set was empty, so nothing could govern.
    EmptyCombinationSet,
}

impl fmt::Display for SniError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPositive { parameter, value } => {
                write!(f, "{parameter} must be greater than zero, got {value}")
            }
            Self::Negative { parameter, value } => {
                write!(f, "{parameter} must not be negative, got {value}")
            }
            Self::NotFinite { parameter } => {
                write!(f, "{parameter} must be a finite number")
            }
            Self::OutOfRange {
                parameter,
                value,
                min,
                max,
            } => write!(
                f,
                "{parameter} must lie between {min} and {max}, got {value}"
            ),
            Self::InconsistentGeometry { detail } => {
                write!(f, "inconsistent section geometry: {detail}")
            }
            Self::UnsupportedSection { reason } => {
                write!(
                    f,
                    "section not covered by the implemented clauses: {reason}"
                )
            }
            Self::EmptyCombinationSet => {
                write!(
                    f,
                    "load combination set is empty, no combination can govern"
                )
            }
        }
    }
}

impl std::error::Error for SniError {}

/// Return the value if it is finite and strictly positive, otherwise an error.
pub(crate) fn positive(parameter: &'static str, value: f64) -> Result<f64> {
    if !value.is_finite() {
        return Err(SniError::NotFinite { parameter });
    }
    if value <= 0.0 {
        return Err(SniError::NonPositive { parameter, value });
    }
    Ok(value)
}

/// Return the value if it is finite and not negative, otherwise an error.
pub(crate) fn non_negative(parameter: &'static str, value: f64) -> Result<f64> {
    if !value.is_finite() {
        return Err(SniError::NotFinite { parameter });
    }
    if value < 0.0 {
        return Err(SniError::Negative { parameter, value });
    }
    Ok(value)
}

/// Return the value if it is finite, otherwise an error.
pub(crate) fn finite(parameter: &'static str, value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(SniError::NotFinite { parameter })
    }
}

/// Return the value if it is finite and within the inclusive bounds.
pub(crate) fn in_range(parameter: &'static str, value: f64, min: f64, max: f64) -> Result<f64> {
    if !value.is_finite() {
        return Err(SniError::NotFinite { parameter });
    }
    if value < min || value > max {
        return Err(SniError::OutOfRange {
            parameter,
            value,
            min,
            max,
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_rejects_zero_and_negative() {
        assert_eq!(
            positive("span", 0.0),
            Err(SniError::NonPositive {
                parameter: "span",
                value: 0.0
            })
        );
        assert!(matches!(
            positive("area", -5.0),
            Err(SniError::NonPositive { .. })
        ));
        assert_eq!(positive("span", 6000.0), Ok(6000.0));
    }

    #[test]
    fn non_finite_is_rejected_before_sign() {
        assert_eq!(
            non_negative("wind", f64::NAN),
            Err(SniError::NotFinite { parameter: "wind" })
        );
        assert!(matches!(
            positive("e", f64::INFINITY),
            Err(SniError::NotFinite { .. })
        ));
    }

    #[test]
    fn range_check_is_inclusive() {
        assert_eq!(in_range("cb", 1.0, 1.0, 3.0), Ok(1.0));
        assert_eq!(in_range("cb", 3.0, 1.0, 3.0), Ok(3.0));
        assert!(matches!(
            in_range("cb", 3.5, 1.0, 3.0),
            Err(SniError::OutOfRange { .. })
        ));
    }

    #[test]
    fn errors_display_readably() {
        let e = SniError::UnsupportedSection {
            reason: "slender web",
        };
        assert!(e.to_string().contains("slender web"));
        assert!(finite("x", 1.0).is_ok());
    }
}
