//! A small catalogue of hot-rolled WF / H profiles common in Indonesia.
//!
//! # Source of the data
//!
//! The dimensions and section properties here are **manufacturer catalogue data**
//! for the JIS G3192 / SNI 07-0329 hot-rolled H and WF series, as published by
//! Indonesian mills and stockists. They are not reproduced from the text of any
//! SNI document. Depth, flange width, thicknesses, fillet radius, area, `Ix`, `Iy`
//! and `Sx` are the published catalogue values.
//!
//! `Zx` is **not** taken from a catalogue. It is computed from the plate geometry
//! with [`SteelSection::plastic_modulus_from_geometry`], ignoring root fillets,
//! which understates `Zx` by roughly one per cent and is therefore conservative.
//!
//! Rolling tolerances and mill practice vary. Verify the profile against the mill
//! certificate for the steel actually being supplied before relying on a result.
//!
//! Units: mm for dimensions, mm2 / mm3 / mm4 for properties. See [`crate::units`].

use crate::steel::section::SteelSection;
use crate::units::{Area, Length, SecondMoment, SectionModulus};

/// One catalogue row, in the units the catalogue publishes.
struct Row {
    designation: &'static str,
    /// Overall depth, mm.
    d: f64,
    /// Flange width, mm.
    bf: f64,
    /// Web thickness, mm.
    tw: f64,
    /// Flange thickness, mm.
    tf: f64,
    /// Root fillet radius, mm.
    r: f64,
    /// Area, cm2.
    area_cm2: f64,
    /// Strong axis second moment, cm4.
    ix_cm4: f64,
    /// Weak axis second moment, cm4.
    iy_cm4: f64,
    /// Strong axis elastic section modulus, cm3.
    sx_cm3: f64,
}

/// The bundled profiles: the narrow flange WF series, then the wide flange H
/// series that is normally used for columns.
///
/// Laid out one profile per line, so the table can be read and checked against a
/// mill catalogue at a glance.
#[rustfmt::skip]
const CATALOGUE: [Row; 20] = [
    // Narrow flange WF series.
    Row { designation: "WF 150x75x5x7",     d:  150.0, bf:  75.0, tw:  5.0, tf:  7.0, r:  8.0, area_cm2:  17.85, ix_cm4:     666.0, iy_cm4:     49.5, sx_cm3:    88.8 },
    Row { designation: "WF 200x100x5.5x8",  d:  200.0, bf: 100.0, tw:  5.5, tf:  8.0, r: 11.0, area_cm2:  27.16, ix_cm4:    1840.0, iy_cm4:    134.0, sx_cm3:   184.0 },
    Row { designation: "WF 250x125x6x9",    d:  250.0, bf: 125.0, tw:  6.0, tf:  9.0, r: 12.0, area_cm2:  37.66, ix_cm4:    4050.0, iy_cm4:    294.0, sx_cm3:   324.0 },
    Row { designation: "WF 300x150x6.5x9",  d:  300.0, bf: 150.0, tw:  6.5, tf:  9.0, r: 13.0, area_cm2:  46.78, ix_cm4:    7210.0, iy_cm4:    508.0, sx_cm3:   481.0 },
    Row { designation: "WF 350x175x7x11",   d:  350.0, bf: 175.0, tw:  7.0, tf: 11.0, r: 14.0, area_cm2:  63.14, ix_cm4:   13600.0, iy_cm4:    984.0, sx_cm3:   775.0 },
    Row { designation: "WF 400x200x8x13",   d:  400.0, bf: 200.0, tw:  8.0, tf: 13.0, r: 16.0, area_cm2:  84.12, ix_cm4:   23700.0, iy_cm4:   1740.0, sx_cm3:  1190.0 },
    Row { designation: "WF 450x200x9x14",   d:  450.0, bf: 200.0, tw:  9.0, tf: 14.0, r: 18.0, area_cm2:  96.76, ix_cm4:   33500.0, iy_cm4:   1870.0, sx_cm3:  1490.0 },
    Row { designation: "WF 500x200x10x16",  d:  500.0, bf: 200.0, tw: 10.0, tf: 16.0, r: 20.0, area_cm2: 114.20, ix_cm4:   47800.0, iy_cm4:   2140.0, sx_cm3:  1910.0 },
    Row { designation: "WF 588x300x12x20",  d:  588.0, bf: 300.0, tw: 12.0, tf: 20.0, r: 28.0, area_cm2: 192.50, ix_cm4:  118000.0, iy_cm4:   9020.0, sx_cm3:  4020.0 },
    Row { designation: "WF 600x200x11x17",  d:  600.0, bf: 200.0, tw: 11.0, tf: 17.0, r: 22.0, area_cm2: 134.40, ix_cm4:   77600.0, iy_cm4:   2280.0, sx_cm3:  2590.0 },
    Row { designation: "WF 700x300x13x24",  d:  700.0, bf: 300.0, tw: 13.0, tf: 24.0, r: 28.0, area_cm2: 235.50, ix_cm4:  201000.0, iy_cm4:  10800.0, sx_cm3:  5760.0 },
    Row { designation: "WF 800x300x14x26",  d:  800.0, bf: 300.0, tw: 14.0, tf: 26.0, r: 28.0, area_cm2: 267.40, ix_cm4:  292000.0, iy_cm4:  11700.0, sx_cm3:  7290.0 },
    Row { designation: "WF 900x300x16x28",  d:  900.0, bf: 300.0, tw: 16.0, tf: 28.0, r: 28.0, area_cm2: 309.80, ix_cm4:  411000.0, iy_cm4:  12600.0, sx_cm3:  9140.0 },
    // Wide flange H series.
    Row { designation: "H 100x100x6x8",     d:  100.0, bf: 100.0, tw:  6.0, tf:  8.0, r: 10.0, area_cm2:  21.90, ix_cm4:     383.0, iy_cm4:    134.0, sx_cm3:    76.5 },
    Row { designation: "H 125x125x6.5x9",   d:  125.0, bf: 125.0, tw:  6.5, tf:  9.0, r: 10.0, area_cm2:  30.31, ix_cm4:     847.0, iy_cm4:    293.0, sx_cm3:   136.0 },
    Row { designation: "H 150x150x7x10",    d:  150.0, bf: 150.0, tw:  7.0, tf: 10.0, r: 11.0, area_cm2:  40.14, ix_cm4:    1640.0, iy_cm4:    563.0, sx_cm3:   219.0 },
    Row { designation: "H 200x200x8x12",    d:  200.0, bf: 200.0, tw:  8.0, tf: 12.0, r: 13.0, area_cm2:  63.53, ix_cm4:    4720.0, iy_cm4:   1600.0, sx_cm3:   472.0 },
    Row { designation: "H 250x250x9x14",    d:  250.0, bf: 250.0, tw:  9.0, tf: 14.0, r: 16.0, area_cm2:  92.18, ix_cm4:   10800.0, iy_cm4:   3650.0, sx_cm3:   867.0 },
    Row { designation: "H 300x300x10x15",   d:  300.0, bf: 300.0, tw: 10.0, tf: 15.0, r: 18.0, area_cm2: 119.80, ix_cm4:   20400.0, iy_cm4:   6750.0, sx_cm3:  1360.0 },
    Row { designation: "H 350x350x12x19",   d:  350.0, bf: 350.0, tw: 12.0, tf: 19.0, r: 20.0, area_cm2: 173.90, ix_cm4:   40300.0, iy_cm4:  13600.0, sx_cm3:  2300.0 },
];

impl Row {
    fn to_section(&self) -> SteelSection {
        let depth = Length::mm(self.d);
        let flange_width = Length::mm(self.bf);
        let web_thickness = Length::mm(self.tw);
        let flange_thickness = Length::mm(self.tf);
        SteelSection {
            designation: self.designation.to_owned(),
            depth,
            flange_width,
            web_thickness,
            flange_thickness,
            fillet_radius: Length::mm(self.r),
            area: Area::cm2(self.area_cm2),
            ix: SecondMoment::cm4(self.ix_cm4),
            iy: SecondMoment::cm4(self.iy_cm4),
            sx: SectionModulus::cm3(self.sx_cm3),
            zx: SteelSection::plastic_modulus_from_geometry(
                depth,
                flange_width,
                web_thickness,
                flange_thickness,
            ),
        }
    }
}

/// Look up a bundled profile by its designation.
///
/// The match is case insensitive and ignores spaces, so `"WF 400x200x8x13"`,
/// `"wf400x200x8x13"` and `"WF400X200X8X13"` all resolve to the same profile.
/// Returns `None` if the designation is not in the catalogue; build a
/// [`SteelSection`] by struct literal for anything else.
///
/// # Examples
///
/// ```
/// use sni_struct::steel::profiles;
///
/// let s = profiles::wf("wf 400x200x8x13").expect("bundled profile");
/// assert_eq!(s.depth.to_mm(), 400.0);
/// assert!(profiles::wf("WF 999x999").is_none());
/// ```
pub fn wf(designation: &str) -> Option<SteelSection> {
    let wanted = normalise(designation);
    CATALOGUE
        .iter()
        .find(|row| normalise(row.designation) == wanted)
        .map(Row::to_section)
}

/// Every designation in the bundled catalogue, in catalogue order.
///
/// # Examples
///
/// ```
/// use sni_struct::steel::profiles;
///
/// assert_eq!(profiles::designations().len(), 20);
/// ```
pub fn designations() -> Vec<&'static str> {
    CATALOGUE.iter().map(|row| row.designation).collect()
}

/// Every bundled profile as a [`SteelSection`].
pub fn all() -> Vec<SteelSection> {
    CATALOGUE.iter().map(Row::to_section).collect()
}

fn normalise(designation: &str) -> String {
    designation
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_space_and_case_insensitive() {
        let a = wf("WF 400x200x8x13").expect("a");
        let b = wf("wf400X200x8X13").expect("b");
        assert_eq!(a, b);
    }

    #[test]
    fn unknown_designation_is_none() {
        assert!(wf("IPE 300").is_none());
    }

    #[test]
    fn every_bundled_profile_validates() {
        for section in all() {
            section
                .validate()
                .unwrap_or_else(|e| panic!("{} failed: {e}", section.designation));
        }
    }

    #[test]
    fn catalogue_holds_twenty_profiles() {
        assert_eq!(designations().len(), 20);
        assert_eq!(all().len(), 20);
    }

    #[test]
    fn plastic_modulus_exceeds_elastic_modulus() {
        // Shape factor Zx/Sx for an I section is about 1.10 to 1.15.
        for section in all() {
            let shape_factor = section.zx.to_mm3() / section.sx.to_mm3();
            assert!(
                (1.05..1.25).contains(&shape_factor),
                "{} shape factor {shape_factor}",
                section.designation
            );
        }
    }
}
