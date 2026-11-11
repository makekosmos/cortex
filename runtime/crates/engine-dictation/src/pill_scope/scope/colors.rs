//! Dot scope's colours for one frame.

use super::Pose;

/// Dot scope's colours for one frame, sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub base: [f32; 3],
    pub core: [f32; 3],
    pub plate: [f32; 4],
    pub rim: [f32; 4],
    pub shadow: [f32; 4],
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn saturate(color: [f32; 3], amount: f32) -> [f32; 3] {
    let luma = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
    color.map(|c| (luma + (c - luma) * amount).clamp(0.0, 1.0))
}

/// Warm pearl while recording, cooling to agent blue while processing, and
/// coral on failure.
pub fn palette(dark: bool, pose: &Pose, saturation: f32) -> Palette {
    let (you, you_core, agent, agent_core, coral, coral_core) = if dark {
        (
            [0.86, 0.83, 0.77],
            [0.98, 0.96, 0.92],
            [0.70, 0.78, 0.90],
            [0.92, 0.95, 1.0],
            [1.0, 0.38, 0.31],
            [1.0, 0.66, 0.6],
        )
    } else {
        (
            [0.62, 0.53, 0.40],
            [0.47, 0.38, 0.26],
            [0.40, 0.47, 0.58],
            [0.24, 0.31, 0.43],
            [0.9, 0.27, 0.21],
            [0.7, 0.18, 0.13],
        )
    };
    let base = mix3(mix3(you, agent, pose.cool), coral, pose.fail);
    let core = mix3(mix3(you_core, agent_core, pose.cool), coral_core, pose.fail);
    let (plate, rim, shadow) = if dark {
        (
            [17.0 / 255.0, 17.0 / 255.0, 20.0 / 255.0, 0.94],
            [1.0, 1.0, 1.0, 0.11],
            [0.0, 0.0, 0.0, 0.5],
        )
    } else {
        (
            [252.0 / 255.0, 251.0 / 255.0, 249.0 / 255.0, 0.96],
            [0.0, 0.0, 0.0, 0.09],
            [20.0 / 255.0, 24.0 / 255.0, 40.0 / 255.0, 0.2],
        )
    };
    Palette {
        base: saturate(base, saturation),
        core: saturate(core, saturation),
        plate,
        rim,
        shadow,
    }
}
