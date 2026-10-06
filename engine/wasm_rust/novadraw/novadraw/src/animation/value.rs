use std::time::Duration;

use crate::{Affine2D, Color, Dimension, Point, Rectangle};

use super::AnimationError;

/// A validated, backend-neutral value that can be sampled by a typed animation track.
pub trait AnimationValue: Clone + PartialEq + Send + Sync + 'static {
    /// Returns whether the value can safely enter an animation plan.
    fn is_valid(&self) -> bool;

    /// Interpolates from `self` to `target` at a clamped progress in `[0, 1]`.
    fn interpolate(&self, target: &Self, progress: f64) -> Self;
}

impl AnimationValue for f64 {
    fn is_valid(&self) -> bool {
        self.is_finite()
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        self + (target - self) * progress
    }
}

impl AnimationValue for Color {
    fn is_valid(&self) -> bool {
        [self.red(), self.green(), self.blue(), self.alpha()]
            .into_iter()
            .all(f64::is_finite)
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Color::rgba(
            self.red() + (target.red() - self.red()) * progress,
            self.green() + (target.green() - self.green()) * progress,
            self.blue() + (target.blue() - self.blue()) * progress,
            self.alpha() + (target.alpha() - self.alpha()) * progress,
        )
    }
}

impl AnimationValue for Point {
    fn is_valid(&self) -> bool {
        self.x().is_finite() && self.y().is_finite()
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        self.lerp(*target, progress)
    }
}

impl AnimationValue for Dimension {
    fn is_valid(&self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Dimension::new(
            self.width + (target.width - self.width) * progress,
            self.height + (target.height - self.height) * progress,
        )
    }
}

impl AnimationValue for Rectangle {
    fn is_valid(&self) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            self.x + self.width,
            self.y + self.height,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.width >= 0.0
            && self.height >= 0.0
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        Rectangle::new(
            self.x + (target.x - self.x) * progress,
            self.y + (target.y - self.y) * progress,
            self.width + (target.width - self.width) * progress,
            self.height + (target.height - self.height) * progress,
        )
    }
}

impl AnimationValue for Affine2D {
    fn is_valid(&self) -> bool {
        self.coeffs().into_iter().all(f64::is_finite)
    }

    fn interpolate(&self, target: &Self, progress: f64) -> Self {
        let from = self.coeffs();
        let to = target.coeffs();
        Affine2D::new(
            from[0] + (to[0] - from[0]) * progress,
            from[1] + (to[1] - from[1]) * progress,
            from[2] + (to[2] - from[2]) * progress,
            from[3] + (to[3] - from[3]) * progress,
            from[4] + (to[4] - from[4]) * progress,
            from[5] + (to[5] - from[5]) * progress,
        )
    }
}

/// Time mapping for a Tween.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum Easing {
    /// Constant-rate interpolation.
    #[default]
    Linear,
    /// Quadratic acceleration.
    EaseIn,
    /// Quadratic deceleration.
    EaseOut,
    /// Symmetric quadratic acceleration and deceleration.
    EaseInOut,
}

impl Easing {
    fn sample(self, progress: f64) -> f64 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::EaseIn => progress * progress,
            Self::EaseOut => 1.0 - (1.0 - progress) * (1.0 - progress),
            Self::EaseInOut if progress < 0.5 => 2.0 * progress * progress,
            Self::EaseInOut => 1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0,
        }
    }
}

/// Finite interpolation between a start and terminal value.
#[derive(Clone, Debug, PartialEq)]
pub struct Tween<V> {
    start: Option<V>,
    end: V,
    duration_micros: u64,
    easing: Easing,
}

impl<V: AnimationValue> Tween<V> {
    /// Creates a Tween with an explicit start and terminal value.
    pub fn between(start: V, end: V, duration: Duration) -> Result<Self, AnimationError> {
        if !start.is_valid() || !end.is_valid() {
            return Err(AnimationError::InvalidValue);
        }
        Ok(Self {
            start: Some(start),
            end,
            duration_micros: duration_micros(duration)?,
            easing: Easing::Linear,
        })
    }

    /// Creates a Tween whose start is resolved from the current presentation value at admission.
    pub fn to(end: V, duration: Duration) -> Result<Self, AnimationError> {
        if !end.is_valid() {
            return Err(AnimationError::InvalidValue);
        }
        Ok(Self {
            start: None,
            end,
            duration_micros: duration_micros(duration)?,
            easing: Easing::Linear,
        })
    }

    /// Sets the time mapping.
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

/// Bounded damped spring interpolation.
#[derive(Clone, Debug, PartialEq)]
pub struct Spring<V> {
    start: Option<V>,
    end: V,
    mass: f64,
    stiffness: f64,
    damping: f64,
    tolerance: f64,
    max_duration_micros: u64,
}

impl<V: AnimationValue> Spring<V> {
    /// Creates a spring with an explicit start and terminal value.
    #[allow(clippy::too_many_arguments)]
    pub fn between(
        start: V,
        end: V,
        mass: f64,
        stiffness: f64,
        damping: f64,
        tolerance: f64,
        max_duration: Duration,
    ) -> Result<Self, AnimationError> {
        validate_spring(&start, &end, mass, stiffness, damping, tolerance)?;
        Ok(Self {
            start: Some(start),
            end,
            mass,
            stiffness,
            damping,
            tolerance,
            max_duration_micros: nonzero_duration_micros(max_duration)
                .map_err(|_| AnimationError::InvalidSpring)?,
        })
    }

    /// Creates a spring whose start is resolved from current presentation.
    pub fn to(
        end: V,
        mass: f64,
        stiffness: f64,
        damping: f64,
        tolerance: f64,
        max_duration: Duration,
    ) -> Result<Self, AnimationError> {
        validate_spring(&end, &end, mass, stiffness, damping, tolerance)?;
        Ok(Self {
            start: None,
            end,
            mass,
            stiffness,
            damping,
            tolerance,
            max_duration_micros: nonzero_duration_micros(max_duration)
                .map_err(|_| AnimationError::InvalidSpring)?,
        })
    }
}

fn validate_spring<V: AnimationValue>(
    start: &V,
    end: &V,
    mass: f64,
    stiffness: f64,
    damping: f64,
    tolerance: f64,
) -> Result<(), AnimationError> {
    if !start.is_valid()
        || !end.is_valid()
        || !mass.is_finite()
        || mass <= 0.0
        || !stiffness.is_finite()
        || stiffness <= 0.0
        || !damping.is_finite()
        || damping < 0.0
        || !tolerance.is_finite()
        || tolerance <= 0.0
        || !(stiffness / mass).is_finite()
        || !(damping / (2.0 * mass)).is_finite()
    {
        return Err(AnimationError::InvalidSpring);
    }
    Ok(())
}

/// Finite exponential approach to a committed terminal value.
#[derive(Clone, Debug, PartialEq)]
pub struct Decay<V> {
    start: Option<V>,
    end: V,
    rate: f64,
    max_duration_micros: u64,
}

impl<V: AnimationValue> Decay<V> {
    /// Creates a decay with an explicit start and terminal value.
    pub fn between(
        start: V,
        end: V,
        rate: f64,
        max_duration: Duration,
    ) -> Result<Self, AnimationError> {
        validate_decay(&start, &end, rate)?;
        let max_duration_micros = decay_duration_micros(rate, max_duration)?;
        Ok(Self {
            start: Some(start),
            end,
            rate,
            max_duration_micros,
        })
    }

    /// Creates a decay whose start is resolved from current presentation.
    pub fn to(end: V, rate: f64, max_duration: Duration) -> Result<Self, AnimationError> {
        validate_decay(&end, &end, rate)?;
        let max_duration_micros = decay_duration_micros(rate, max_duration)?;
        Ok(Self {
            start: None,
            end,
            rate,
            max_duration_micros,
        })
    }
}

fn validate_decay<V: AnimationValue>(start: &V, end: &V, rate: f64) -> Result<(), AnimationError> {
    if !start.is_valid() || !end.is_valid() || !rate.is_finite() || rate <= 0.0 {
        return Err(AnimationError::InvalidDecay);
    }
    Ok(())
}

/// One normalized keyframe.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe<V> {
    offset: f64,
    value: V,
}

impl<V> Keyframe<V> {
    /// Creates a keyframe at a normalized offset.
    pub fn new(offset: f64, value: V) -> Self {
        Self { offset, value }
    }
}

/// Finite piecewise-linear keyframe motion.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes<V> {
    duration_micros: u64,
    frames: Vec<Keyframe<V>>,
}

impl<V: AnimationValue> Keyframes<V> {
    /// Validates and creates a keyframe motion.
    pub fn new(duration: Duration, frames: Vec<Keyframe<V>>) -> Result<Self, AnimationError> {
        if frames.len() < 2 {
            return Err(AnimationError::MissingEndpointKeyframe);
        }
        for (index, frame) in frames.iter().enumerate() {
            if !frame.offset.is_finite()
                || !(0.0..=1.0).contains(&frame.offset)
                || !frame.value.is_valid()
            {
                return Err(if frame.value.is_valid() {
                    AnimationError::InvalidKeyframeOffset { index }
                } else {
                    AnimationError::InvalidValue
                });
            }
            if index > 0 && frame.offset <= frames[index - 1].offset {
                return Err(AnimationError::KeyframesOutOfOrder { index });
            }
        }
        if frames.first().map(|frame| frame.offset) != Some(0.0)
            || frames.last().map(|frame| frame.offset) != Some(1.0)
        {
            return Err(AnimationError::MissingEndpointKeyframe);
        }
        Ok(Self {
            duration_micros: duration_micros(duration)?,
            frames,
        })
    }
}

/// Typed finite motion sampled by a Track.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Motion<V> {
    /// Start/end interpolation.
    Tween(Tween<V>),
    /// Piecewise interpolation through normalized keyframes.
    Keyframes(Keyframes<V>),
    /// Bounded damped spring interpolation.
    Spring(Spring<V>),
    /// Bounded exponential decay.
    Decay(Decay<V>),
}

impl<V: AnimationValue> Motion<V> {
    pub(crate) fn duration_micros(&self) -> u64 {
        match self {
            Self::Tween(tween) => tween.duration_micros,
            Self::Keyframes(keyframes) => keyframes.duration_micros,
            Self::Spring(spring) => spring.max_duration_micros,
            Self::Decay(decay) => decay.max_duration_micros,
        }
    }

    pub(crate) fn prepare(self, current: V) -> PreparedMotion<V> {
        match self {
            Self::Tween(tween) => PreparedMotion::Tween {
                start: tween.start.unwrap_or(current),
                end: tween.end,
                duration_micros: tween.duration_micros,
                easing: tween.easing,
            },
            Self::Keyframes(keyframes) => PreparedMotion::Keyframes(keyframes),
            Self::Spring(spring) => PreparedMotion::Spring {
                start: spring.start.unwrap_or(current),
                end: spring.end,
                mass: spring.mass,
                stiffness: spring.stiffness,
                damping: spring.damping,
                tolerance: spring.tolerance,
                max_duration_micros: spring.max_duration_micros,
            },
            Self::Decay(decay) => PreparedMotion::Decay {
                start: decay.start.unwrap_or(current),
                end: decay.end,
                rate: decay.rate,
                max_duration_micros: decay.max_duration_micros,
            },
        }
    }

    pub(crate) fn terminal_value(&self) -> &V {
        match self {
            Self::Tween(tween) => &tween.end,
            Self::Keyframes(keyframes) => {
                &keyframes
                    .frames
                    .last()
                    .expect("validated keyframes contain an endpoint")
                    .value
            }
            Self::Spring(spring) => &spring.end,
            Self::Decay(decay) => &decay.end,
        }
    }

    pub(crate) fn reversed(&self) -> Result<Self, AnimationError> {
        Ok(match self {
            Self::Tween(tween) => {
                let start = tween
                    .start
                    .as_ref()
                    .ok_or(AnimationError::InvalidComposition)?;
                Self::Tween(Tween {
                    start: Some(tween.end.clone()),
                    end: start.clone(),
                    duration_micros: tween.duration_micros,
                    easing: tween.easing,
                })
            }
            Self::Keyframes(keyframes) => Self::Keyframes(Keyframes {
                duration_micros: keyframes.duration_micros,
                frames: keyframes
                    .frames
                    .iter()
                    .rev()
                    .map(|frame| Keyframe {
                        offset: 1.0 - frame.offset,
                        value: frame.value.clone(),
                    })
                    .collect(),
            }),
            Self::Spring(spring) => {
                let start = spring
                    .start
                    .as_ref()
                    .ok_or(AnimationError::InvalidComposition)?;
                Self::Spring(Spring {
                    start: Some(spring.end.clone()),
                    end: start.clone(),
                    mass: spring.mass,
                    stiffness: spring.stiffness,
                    damping: spring.damping,
                    tolerance: spring.tolerance,
                    max_duration_micros: spring.max_duration_micros,
                })
            }
            Self::Decay(decay) => {
                let start = decay
                    .start
                    .as_ref()
                    .ok_or(AnimationError::InvalidComposition)?;
                Self::Decay(Decay {
                    start: Some(decay.end.clone()),
                    end: start.clone(),
                    rate: decay.rate,
                    max_duration_micros: decay.max_duration_micros,
                })
            }
        })
    }
}

pub(crate) enum PreparedMotion<V> {
    Tween {
        start: V,
        end: V,
        duration_micros: u64,
        easing: Easing,
    },
    Keyframes(Keyframes<V>),
    Spring {
        start: V,
        end: V,
        mass: f64,
        stiffness: f64,
        damping: f64,
        tolerance: f64,
        max_duration_micros: u64,
    },
    Decay {
        start: V,
        end: V,
        rate: f64,
        max_duration_micros: u64,
    },
}

impl<V: AnimationValue> PreparedMotion<V> {
    pub(crate) fn sample(&self, elapsed_micros: u64) -> V {
        match self {
            Self::Tween {
                start,
                end,
                duration_micros,
                easing,
            } => {
                let progress = normalized_progress(elapsed_micros, *duration_micros);
                start.interpolate(end, easing.sample(progress))
            }
            Self::Keyframes(keyframes) => {
                let progress = normalized_progress(elapsed_micros, keyframes.duration_micros);
                let upper = keyframes
                    .frames
                    .partition_point(|frame| frame.offset < progress)
                    .min(keyframes.frames.len() - 1);
                if upper == 0 {
                    return keyframes.frames[0].value.clone();
                }
                let lower = upper - 1;
                let from = &keyframes.frames[lower];
                let to = &keyframes.frames[upper];
                let segment = (progress - from.offset) / (to.offset - from.offset);
                from.value.interpolate(&to.value, segment.clamp(0.0, 1.0))
            }
            Self::Spring {
                start,
                end,
                mass,
                stiffness,
                damping,
                tolerance,
                max_duration_micros,
            } => {
                if elapsed_micros >= *max_duration_micros {
                    return end.clone();
                }
                let time = elapsed_micros as f64 / 1_000_000.0;
                let omega = (stiffness / mass).sqrt();
                let decay = damping / (2.0 * mass);
                let response = if decay < omega {
                    let frequency = (omega * omega - decay * decay).sqrt();
                    (-decay * time).exp()
                        * ((frequency * time).cos() + decay / frequency * (frequency * time).sin())
                } else if (decay - omega).abs() <= f64::EPSILON {
                    (1.0 + omega * time) * (-omega * time).exp()
                } else {
                    let root = (decay * decay - omega * omega).sqrt();
                    let first = -decay + root;
                    let second = -decay - root;
                    let first_coefficient = -second / (first - second);
                    let second_coefficient = first / (first - second);
                    first_coefficient * (first * time).exp()
                        + second_coefficient * (second * time).exp()
                };
                let progress = 1.0 - response;
                if response.abs() <= *tolerance {
                    end.clone()
                } else {
                    start.interpolate(end, progress.clamp(0.0, 1.0))
                }
            }
            Self::Decay {
                start,
                end,
                rate,
                max_duration_micros,
            } => {
                if elapsed_micros >= *max_duration_micros {
                    return end.clone();
                }
                let duration_seconds = *max_duration_micros as f64 / 1_000_000.0;
                let elapsed_seconds = elapsed_micros as f64 / 1_000_000.0;
                let denominator = 1.0 - (-rate * duration_seconds).exp();
                let progress = (1.0 - (-rate * elapsed_seconds).exp()) / denominator;
                start.interpolate(end, progress.clamp(0.0, 1.0))
            }
        }
    }
}

pub(crate) fn duration_micros(duration: Duration) -> Result<u64, AnimationError> {
    u64::try_from(duration.as_micros()).map_err(|_| AnimationError::InvalidDuration)
}

fn nonzero_duration_micros(duration: Duration) -> Result<u64, AnimationError> {
    let micros = duration_micros(duration)?;
    (micros != 0)
        .then_some(micros)
        .ok_or(AnimationError::InvalidDuration)
}

fn decay_duration_micros(rate: f64, duration: Duration) -> Result<u64, AnimationError> {
    let micros = nonzero_duration_micros(duration).map_err(|_| AnimationError::InvalidDecay)?;
    let seconds = micros as f64 / 1_000_000.0;
    let denominator = 1.0 - (-rate * seconds).exp();
    if !denominator.is_finite() || denominator <= 0.0 {
        return Err(AnimationError::InvalidDecay);
    }
    Ok(micros)
}

fn normalized_progress(elapsed_micros: u64, duration_micros: u64) -> f64 {
    if duration_micros == 0 {
        1.0
    } else {
        elapsed_micros.min(duration_micros) as f64 / duration_micros as f64
    }
}
