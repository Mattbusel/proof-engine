//! Post-processing passes: bloom, distortion, color grade, motion blur, etc.

pub mod bloom;
pub mod color_grade;
pub mod distortion;
pub mod motion_blur;
pub mod chromatic;
pub mod grain;
pub mod scanlines;
pub mod pipeline;

pub use pipeline::PostFxPipeline;

/// Parameters for the full post-processing pipeline this frame.
#[derive(Clone, Debug)]
pub struct PostFxParams {
    pub bloom: bloom::BloomParams,
    pub color_grade: color_grade::ColorGradeParams,
    pub distortion: distortion::DistortionParams,
    pub motion_blur: motion_blur::MotionBlurParams,
    pub chromatic_aberration: chromatic::ChromaticParams,
    pub film_grain: grain::GrainParams,
    pub scanlines: scanlines::ScanlineParams,
}

impl Default for PostFxParams {
    fn default() -> Self {
        Self {
            bloom: bloom::BloomParams::default(),
            color_grade: color_grade::ColorGradeParams::default(),
            distortion: distortion::DistortionParams::default(),
            motion_blur: motion_blur::MotionBlurParams::default(),
            chromatic_aberration: chromatic::ChromaticParams::default(),
            film_grain: grain::GrainParams::default(),
            scanlines: scanlines::ScanlineParams::default(),
        }
    }
}
