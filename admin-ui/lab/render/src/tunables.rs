//! The settings the canvas itself owns, in one place. The page changes them
//! through `LabCanvas` setters (src/web.rs), which write the copy in `Ui`;
//! on wasm the same struct is a Bevy resource, refreshed from that copy at
//! the start of every frame, and the systems that draw read the resource.
//! The pointer code reads the copy in `Ui`, so it stays testable without
//! Bevy.
//!
//! How fast packets move is not here: the core places every packet
//! (`packets_now`), and the page sets its pace with `set_sim_speed` and
//! `set_clock_speed`.

#[cfg(target_arch = "wasm32")]
use bevy::ecs as bevy_ecs;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(target_arch = "wasm32", derive(bevy::prelude::Resource))]
pub struct Tunables {
    /// Multisampling: 4 or 1. Both cameras draw into one target and use it.
    pub msaa: u32,
    /// Whether the labels are drawn. Off also skips the label camera's pass.
    pub labels: bool,
    /// The zoom range, in canvas pixels per world unit.
    pub zoom_min: f32,
    pub zoom_max: f32,
    /// Zoom per wheel pixel: one notch (about 100 px) is a factor of e^0.15.
    pub wheel_rate: f32,
    /// The camera's angle above the ground, in degrees. 90 looks straight
    /// down and is the only angle the page's camera maths (`Cam`, pan and
    /// zoom about the pointer) is exact for; lower angles tilt the same
    /// scene for a look at it in 3D.
    pub pitch_deg: f32,
    /// How long the zoom has to stay put before the labels are drawn again
    /// at exactly that zoom. While it moves they are scaled from the
    /// nearest quarter step of a power of two, which a wheel turn crosses
    /// only a few times.
    pub label_settle_ms: f64,
}

impl Default for Tunables {
    fn default() -> Self {
        Tunables {
            msaa: 4,
            labels: true,
            zoom_min: 0.3,
            zoom_max: 2.5,
            wheel_rate: 0.0015,
            pitch_deg: 90.0,
            label_settle_ms: 150.0,
        }
    }
}

impl Tunables {
    pub fn clamp_zoom(&self, k: f32) -> f32 {
        k.clamp(self.zoom_min, self.zoom_max)
    }

    /// Any count but 4 means no multisampling.
    pub fn set_msaa(&mut self, samples: u32) {
        self.msaa = if samples >= 4 { 4 } else { 1 };
    }

    /// Between 20 and 90 degrees: below 20 the ground is a sliver.
    pub fn set_pitch(&mut self, deg: f32) {
        self.pitch_deg = if deg.is_finite() {
            deg.clamp(20.0, 90.0)
        } else {
            90.0
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setters_keep_values_the_renderer_can_use() {
        let mut t = Tunables::default();
        assert_eq!((t.msaa, t.pitch_deg), (4, 90.0));
        t.set_msaa(8);
        assert_eq!(t.msaa, 4);
        t.set_msaa(2);
        assert_eq!(t.msaa, 1);
        t.set_pitch(5.0);
        assert_eq!(t.pitch_deg, 20.0);
        t.set_pitch(f32::NAN);
        assert_eq!(t.pitch_deg, 90.0);
        assert_eq!(t.clamp_zoom(9.0), 2.5);
        assert_eq!(t.clamp_zoom(0.1), 0.3);
    }
}
