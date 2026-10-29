//! Internal inverse calculation with explicit numerical failure semantics.

use super::Affine2D;

/// Numerical failure of a checked affine inverse, distinct from mathematical singularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseFailure {
    /// At least one input coefficient is not finite.
    NonFiniteInput,
    /// The mathematical inverse cannot be represented by finite coefficients.
    InverseUnrepresentable,
    /// Inverse calculation failed numerically.
    NumericalFailure,
}

/// An exact finite dyadic value. Products of two f64 significands fit in u128.
#[derive(Clone, Copy)]
struct Dyadic {
    negative: bool,
    significand: u128,
    exponent: i32,
}

impl Dyadic {
    fn from_f64(value: f64) -> Self {
        let bits = value.to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, exponent) = if biased == 0 {
            (fraction as u128, -1074)
        } else {
            (((1_u64 << 52) | fraction) as u128, biased - 1023 - 52)
        };
        Self::normalized(bits >> 63 != 0, significand, exponent)
    }

    fn normalized(negative: bool, significand: u128, exponent: i32) -> Self {
        if significand == 0 {
            return Self {
                negative: false,
                significand: 0,
                exponent: 0,
            };
        }
        let shift = significand.trailing_zeros();
        Self {
            negative,
            significand: significand >> shift,
            exponent: exponent + shift as i32,
        }
    }

    fn product(left: f64, right: f64) -> Self {
        let left = Self::from_f64(left);
        let right = Self::from_f64(right);
        Self::normalized(
            left.negative != right.negative,
            left.significand * right.significand,
            left.exponent + right.exponent,
        )
    }

    fn scaled(self) -> Scaled {
        if self.significand == 0 {
            return Scaled {
                fraction: 0.0,
                exponent: 0,
            };
        }
        let top = 127 - self.significand.leading_zeros() as i32;
        let sign = if self.negative { -1.0 } else { 1.0 };
        Scaled {
            fraction: sign * (self.significand as f64) * 2.0_f64.powi(-top),
            exponent: self.exponent + top,
        }
    }

    fn difference(self, mut other: Self) -> Scaled {
        other.negative = !other.negative;
        if self.significand == 0 {
            return other.scaled();
        }
        if other.significand == 0 {
            return self.scaled();
        }
        let base = self.exponent.min(other.exponent);
        let left_shift = (self.exponent - base) as u32;
        let right_shift = (other.exponent - base) as u32;
        let left_bits = 128 - self.significand.leading_zeros();
        let right_bits = 128 - other.significand.leading_zeros();
        // Exact alignment handles all close cancellation, including determinants
        // whose rounded products are equal. Reserve a bit for addition.
        if left_bits + left_shift < 128 && right_bits + right_shift < 128 {
            let left = self.significand << left_shift;
            let right = other.significand << right_shift;
            let (negative, significand) = if self.negative == other.negative {
                (self.negative, left + right)
            } else if left >= right {
                (self.negative, left - right)
            } else {
                (other.negative, right - left)
            };
            return Self::normalized(negative, significand, base).scaled();
        }
        // In this branch the magnitudes cannot nearly cancel: the original
        // products have at most 106 bits, but aligned values need >=128 bits.
        let left = self.scaled();
        let right = other.scaled();
        let exponent = left.exponent.max(right.exponent);
        Scaled {
            fraction: left.fraction * 2.0_f64.powi(left.exponent - exponent)
                + right.fraction * 2.0_f64.powi(right.exponent - exponent),
            exponent,
        }
    }
}

#[derive(Clone, Copy)]
struct Scaled {
    fraction: f64,
    exponent: i32,
}

fn ratio(numerator: Scaled, denominator: Scaled) -> Result<f64, InverseFailure> {
    let mut value = numerator.fraction / denominator.fraction;
    if !value.is_finite() {
        return Err(InverseFailure::NumericalFailure);
    }
    let mut exponent = numerator.exponent - denominator.exponent;
    // Intermediate powers must themselves stay representable. Applying negative
    // powers in chunks also retains subnormal results.
    const EXPONENT_CHUNK: i32 = 512;
    while exponent > EXPONENT_CHUNK {
        value *= 2.0_f64.powi(EXPONENT_CHUNK);
        exponent -= EXPONENT_CHUNK;
    }
    while exponent < -EXPONENT_CHUNK {
        value *= 2.0_f64.powi(-EXPONENT_CHUNK);
        exponent += EXPONENT_CHUNK;
    }
    value *= 2.0_f64.powi(exponent);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(InverseFailure::InverseUnrepresentable)
    }
}

impl Affine2D {
    /// Returns `None` only for mathematical singularity, distinguishing numerical failures.
    pub fn checked_inverse(self) -> Result<Option<Self>, InverseFailure> {
        let [a, b, c, d, e, f] = self.coeffs();
        if ![a, b, c, d, e, f].into_iter().all(f64::is_finite) {
            return Err(InverseFailure::NonFiniteInput);
        }
        let determinant = Dyadic::product(a, d).difference(Dyadic::product(b, c));
        if determinant.fraction == 0.0 {
            return Ok(None);
        }
        let numerators = [
            Dyadic::from_f64(d).scaled(),
            Dyadic::from_f64(-b).scaled(),
            Dyadic::from_f64(-c).scaled(),
            Dyadic::from_f64(a).scaled(),
            Dyadic::product(c, f).difference(Dyadic::product(d, e)),
            Dyadic::product(b, e).difference(Dyadic::product(a, f)),
        ];
        let [a, b, c, d, e, f] = numerators.map(|value| ratio(value, determinant));
        Ok(Some(Self::new(a?, b?, c?, d?, e?, f?)))
    }
}

#[cfg(test)]
mod s07_checked_inverse_tests {
    use super::*;

    #[test]
    fn classifies_exact_singularity_and_range_failures() {
        assert_eq!(
            Affine2D::new(1.0, 2.0, 2.0, 4.0, 0.0, 0.0).checked_inverse(),
            Ok(None)
        );
        assert_eq!(
            Affine2D::from_scale(1e-320, 1.0).checked_inverse(),
            Err(InverseFailure::InverseUnrepresentable)
        );
        assert_eq!(
            Affine2D::new(0.0, 0.0, 0.0, 0.0, f64::NAN, 0.0).checked_inverse(),
            Err(InverseFailure::NonFiniteInput)
        );
        assert_eq!(
            Affine2D::new(1e-200, 0.0, 0.0, 1.0, 1e200, 0.0).checked_inverse(),
            Err(InverseFailure::InverseUnrepresentable)
        );
    }

    #[test]
    fn avoids_intermediate_determinant_range_limits() {
        for (x, y) in [(1e200, 1e200), (1e-200, 1e-200), (1e200, 1e-200)] {
            let inverse = Affine2D::from_scale(x, y)
                .checked_inverse()
                .unwrap()
                .unwrap();
            let coefficients = inverse.coeffs();
            assert!((coefficients[0] * x - 1.0).abs() < 1e-14);
            assert!((coefficients[3] * y - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn distinguishes_exact_cancellation_from_rounded_product_equality() {
        let adjacent = f64::from_bits(1.0_f64.to_bits() + 1);
        let transform = Affine2D::new(adjacent, 1.0, 1.0, 2.0 - adjacent, 0.0, 0.0);
        assert_eq!(transform.determinant(), 0.0);
        assert!(transform.checked_inverse().unwrap().is_some());
        // Translation products overflow individually but cancel exactly.
        let transform = Affine2D::new(1e200, 0.0, 1e200, 1e200, 1e200, 1e200);
        let inverse = transform.checked_inverse().unwrap().unwrap();
        assert_eq!(inverse.coeffs()[4], 0.0);
        assert!((inverse.coeffs()[5] + 1.0).abs() < 1e-14);
    }
}
