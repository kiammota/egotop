use serde::{Deserialize, Deserializer, Serialize};

/// Unified easing curves supported by timeline keyframes and declarative transitions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EasingCurve {
    Linear,
    EaseIn,
    EaseOut,
    #[default]
    EaseInOut,
    CubicBezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
}

impl EasingCurve {
    /// Evaluates the easing curve for normalized time `t` in `[0.0, 1.0]`.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::CubicBezier { x1, y1, x2, y2 } => eval_cubic_bezier(t, x1, y1, x2, y2),
        }
    }

    /// Parses string representations such as `"linear"`, `"ease_in_out"`, or
    /// `"cubic_bezier(0.25, 0.1, 0.25, 1.0)"`.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let s = raw.trim().to_ascii_lowercase();
        match s.as_str() {
            "linear" => Ok(Self::Linear),
            "ease_in" | "ease-in" | "in" => Ok(Self::EaseIn),
            "ease_out" | "ease-out" | "out" => Ok(Self::EaseOut),
            "ease_in_out" | "ease-in-out" | "in_out" | "smooth" => Ok(Self::EaseInOut),
            _ => {
                let inner = s
                    .strip_prefix("cubic_bezier(")
                    .or_else(|| s.strip_prefix("cubic-bezier("))
                    .and_then(|rest| rest.strip_suffix(')'))
                    .ok_or_else(|| format!("Unknown easing curve '{raw}'"))?;
                let parts: Vec<f32> = inner
                    .split(',')
                    .map(|p| {
                        p.trim()
                            .parse::<f32>()
                            .map_err(|_| format!("Invalid cubic_bezier number '{p}'"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if parts.len() != 4 {
                    return Err(format!(
                        "cubic_bezier requires 4 control numbers, found {}",
                        parts.len()
                    ));
                }
                let x1 = parts[0].clamp(0.0, 1.0);
                let x2 = parts[2].clamp(0.0, 1.0);
                Ok(Self::CubicBezier {
                    x1,
                    y1: parts[1],
                    x2,
                    y2: parts[3],
                })
            }
        }
    }
}

impl Serialize for EasingCurve {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::Linear => serializer.serialize_str("linear"),
            Self::EaseIn => serializer.serialize_str("ease_in"),
            Self::EaseOut => serializer.serialize_str("ease_out"),
            Self::EaseInOut => serializer.serialize_str("ease_in_out"),
            Self::CubicBezier { x1, y1, x2, y2 } => {
                serializer.serialize_str(&format!("cubic_bezier({x1:.3},{y1:.3},{x2:.3},{y2:.3})"))
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EasingInput {
    Text(String),
    Table {
        #[serde(rename = "type")]
        kind: String,
        #[serde(default)]
        points: [f32; 4],
    },
}

impl<'de> Deserialize<'de> for EasingCurve {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match EasingInput::deserialize(deserializer)? {
            EasingInput::Text(s) => Self::parse(&s).map_err(serde::de::Error::custom),
            EasingInput::Table { kind, points } => {
                if kind.eq_ignore_ascii_case("cubic_bezier")
                    || kind.eq_ignore_ascii_case("cubic-bezier")
                {
                    Ok(Self::CubicBezier {
                        x1: points[0].clamp(0.0, 1.0),
                        y1: points[1],
                        x2: points[2].clamp(0.0, 1.0),
                        y2: points[3],
                    })
                } else {
                    Self::parse(&kind).map_err(serde::de::Error::custom)
                }
            }
        }
    }
}

fn cubic_bezier_sample(u: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - u;
    3.0 * inv * inv * u * p1 + 3.0 * inv * u * u * p2 + u * u * u
}

fn cubic_bezier_derivative(u: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - u;
    3.0 * inv * inv * p1 + 6.0 * inv * u * (p2 - p1) + 3.0 * u * u * (1.0 - p2)
}

/// Solves `B_x(u) = target_x` using Newton-Raphson with bisection fallback and
/// returns `B_y(u)`.
fn eval_cubic_bezier(target_x: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    if target_x <= 0.0 {
        return 0.0;
    }
    if target_x >= 1.0 {
        return 1.0;
    }
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    let mut u = target_x;

    for _ in 0..8 {
        let x = cubic_bezier_sample(u, x1, x2) - target_x;
        if x.abs() < 1e-5 {
            return cubic_bezier_sample(u, y1, y2);
        }
        if x < 0.0 {
            lo = u;
        } else {
            hi = u;
        }
        let dx = cubic_bezier_derivative(u, x1, x2);
        if dx.abs() > 1e-5 {
            let next = u - x / dx;
            if next > lo && next < hi {
                u = next;
                continue;
            }
        }
        u = 0.5 * (lo + hi);
    }

    for _ in 0..10 {
        let x = cubic_bezier_sample(u, x1, x2);
        if (x - target_x).abs() < 1e-5 {
            break;
        }
        if x < target_x {
            lo = u;
        } else {
            hi = u;
        }
        u = 0.5 * (lo + hi);
    }

    cubic_bezier_sample(u, y1, y2)
}
