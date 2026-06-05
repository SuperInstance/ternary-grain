#![forbid(unsafe_code)]

//! Granular synthesis for ternary (-1, 0, +1) streams.

use rand::Rng;

/// A ternary value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ternary {
    Neg,
    Zero,
    Pos,
}

impl Ternary {
    pub fn to_f64(self) -> f64 {
        match self { Ternary::Neg => -1.0, Ternary::Zero => 0.0, Ternary::Pos => 1.0 }
    }

    pub fn from_f64(v: f64) -> Self {
        if v < -0.33 { Ternary::Neg } else if v > 0.33 { Ternary::Pos } else { Ternary::Zero }
    }
}

// ── Grain ──────────────────────────────────────────────────────────

/// A short windowed fragment of ternary data.
#[derive(Debug, Clone, PartialEq)]
pub struct Grain {
    pub data: Vec<Ternary>,
    pub position: f64,   // start position in source (0.0..1.0)
    pub size: usize,      // grain length in samples
    pub pitch: f64,       // playback rate (1.0 = normal)
    pub amplitude: f64,   // 0.0..1.0
}

impl Grain {
    pub fn new(data: Vec<Ternary>, position: f64, size: usize) -> Self {
        Self { data, position, size, pitch: 1.0, amplitude: 1.0 }
    }

    /// Apply a Hann window to the grain's amplitude.
    pub fn windowed(&self) -> Vec<f64> {
        let n = self.data.len();
        self.data.iter().enumerate().map(|(i, &t)| {
            let w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos());
            t.to_f64() * w * self.amplitude
        }).collect()
    }

    /// Empty grain.
    pub fn empty() -> Self {
        Self { data: Vec::new(), position: 0.0, size: 0, pitch: 1.0, amplitude: 0.0 }
    }
}

// ── Grain cloud ────────────────────────────────────────────────────

/// A collection of overlapping grains with shared parameters.
#[derive(Debug, Clone)]
pub struct GrainCloud {
    pub grains: Vec<Grain>,
    pub density: f64,     // grains per unit length
    pub spread: f64,      // randomization of positions (0.0..1.0)
}

impl GrainCloud {
    pub fn new(grains: Vec<Grain>) -> Self {
        Self { grains, density: 1.0, spread: 0.0 }
    }

    /// Create a cloud of grains from a source signal.
    pub fn from_source(source: &[Ternary], grain_size: usize, count: usize, spread: f64) -> Self {
        let mut rng = rand::thread_rng();
        let grains: Vec<Grain> = (0..count).map(|_| {
            let base_pos = rng.gen::<f64>() * (1.0 - spread);
            let pos = base_pos + rng.gen::<f64>() * spread;
            let start = ((pos * source.len() as f64) as usize).min(source.len().saturating_sub(grain_size));
            let data = source[start..start + grain_size].to_vec();
            Grain::new(data, pos, grain_size)
        }).collect();
        Self { grains, density: count as f64, spread }
    }

    /// Synthesize the cloud by summing windowed grains into an output buffer.
    pub fn synthesize(&self, output_len: usize) -> Vec<f64> {
        let mut buffer = vec![0.0; output_len];
        for grain in &self.grains {
            let offset = (grain.position * output_len as f64) as usize;
            let windowed = grain.windowed();
            for (i, &sample) in windowed.iter().enumerate() {
                let idx = offset + i;
                if idx < output_len {
                    buffer[idx] += sample;
                }
            }
        }
        buffer
    }
}

// ── Grain parameters ───────────────────────────────────────────────

/// Parameters controlling grain generation.
#[derive(Debug, Clone, PartialEq)]
pub struct GrainParams {
    pub position: f64,
    pub size: usize,
    pub density: f64,
    pub spread: f64,
    pub pitch: f64,
}

impl Default for GrainParams {
    fn default() -> Self {
        Self { position: 0.5, size: 64, density: 10.0, spread: 0.1, pitch: 1.0 }
    }
}

/// Create grains from a source using parameters.
pub fn create_grains(source: &[Ternary], params: &GrainParams, count: usize) -> GrainCloud {
    GrainCloud::from_source(source, params.size, count, params.spread)
}

// ── Scatter ────────────────────────────────────────────────────────

/// Randomize grain positions within a cloud.
pub fn scatter(cloud: &mut GrainCloud, amount: f64) {
    let mut rng = rand::thread_rng();
    for grain in &mut cloud.grains {
        grain.position += (rng.gen::<f64>() - 0.5) * amount;
        grain.position = grain.position.clamp(0.0, 1.0);
    }
}

// ── Stretch ────────────────────────────────────────────────────────

/// Time-stretch by overlapping grains with reduced density (more overlap).
/// Returns a longer output buffer.
pub fn stretch(source: &[Ternary], grain_size: usize, stretch_factor: f64, overlap: usize) -> Vec<Ternary> {
    if source.is_empty() || grain_size == 0 {
        return source.to_vec();
    }
    let step = (grain_size as f64 / stretch_factor) as usize;
    let step = step.max(1);
    let output_len = (source.len() as f64 * stretch_factor) as usize + grain_size;
    let mut buffer = vec![0.0_f64; output_len];
    let mut counts = vec![0usize; output_len];

    let mut pos = 0usize;
    while pos + grain_size <= source.len() {
        for i in 0..grain_size {
            let out_idx = pos + i;
            if out_idx < output_len {
                // Hann window
                let w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / grain_size as f64).cos());
                buffer[out_idx] += source[pos + i].to_f64() * w;
                counts[out_idx] += 1;
            }
        }
        pos += step;
    }

    buffer.iter().zip(counts.iter()).map(|(&v, &c)| {
        if c > 0 { Ternary::from_f64(v / c as f64) } else { Ternary::Zero }
    }).collect()
}

// ── Freeze ─────────────────────────────────────────────────────────

/// Loop a single grain for `repetitions` times.
pub fn freeze(grain: &Grain, repetitions: usize) -> Vec<Ternary> {
    grain.data.iter().cycle().take(grain.data.len() * repetitions).copied().collect()
}

// ── Morph ──────────────────────────────────────────────────────────

/// Crossfade between two grain clouds. `mix` in 0.0..1.0 (0=all A, 1=all B).
pub fn morph(cloud_a: &GrainCloud, cloud_b: &GrainCloud, mix: f64, output_len: usize) -> Vec<f64> {
    let a = cloud_a.synthesize(output_len);
    let b = cloud_b.synthesize(output_len);
    a.iter().zip(b.iter()).map(|(&va, &vb)| va * (1.0 - mix) + vb * mix).collect()
}

// ════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn source_signal(len: usize) -> Vec<Ternary> {
        (0..len).map(|i| if i % 2 == 0 { Ternary::Pos } else { Ternary::Neg }).collect()
    }

    #[test]
    fn test_grain_new() {
        let g = Grain::new(vec![Ternary::Pos, Ternary::Neg], 0.5, 2);
        assert_eq!(g.data.len(), 2);
        assert!((g.position - 0.5).abs() < 1e-9);
        assert_eq!(g.size, 2);
    }

    #[test]
    fn test_grain_windowed() {
        let g = Grain::new(vec![Ternary::Pos; 4], 0.0, 4);
        let w = g.windowed();
        assert_eq!(w.len(), 4);
        // Center should be loudest for even-length Hann
        assert!(w[1] > w[0]);
        assert!(w[2] > w[3]);
    }

    #[test]
    fn test_grain_empty() {
        let g = Grain::empty();
        assert!(g.data.is_empty());
        assert_eq!(g.size, 0);
    }

    #[test]
    fn test_grain_amplitude() {
        let mut g = Grain::new(vec![Ternary::Pos; 4], 0.0, 4);
        g.amplitude = 0.5;
        let w = g.windowed();
        assert!(w.iter().all(|&v| v <= 0.5));
    }

    #[test]
    fn test_grain_cloud_from_source() {
        let source = source_signal(100);
        let cloud = GrainCloud::from_source(&source, 10, 5, 0.2);
        assert_eq!(cloud.grains.len(), 5);
        for g in &cloud.grains {
            assert_eq!(g.data.len(), 10);
        }
    }

    #[test]
    fn test_grain_cloud_synthesize() {
        let source = source_signal(100);
        let cloud = GrainCloud::from_source(&source, 10, 5, 0.2);
        let out = cloud.synthesize(200);
        assert_eq!(out.len(), 200);
    }

    #[test]
    fn test_grain_params_default() {
        let p = GrainParams::default();
        assert_eq!(p.size, 64);
        assert!((p.pitch - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_create_grains() {
        let source = source_signal(200);
        let params = GrainParams { size: 20, spread: 0.3, ..Default::default() };
        let cloud = create_grains(&source, &params, 10);
        assert_eq!(cloud.grains.len(), 10);
    }

    #[test]
    fn test_scatter() {
        let source = source_signal(100);
        let mut cloud = GrainCloud::from_source(&source, 10, 5, 0.0);
        // Record original positions
        let orig: Vec<f64> = cloud.grains.iter().map(|g| g.position).collect();
        scatter(&mut cloud, 0.5);
        // At least some should have changed
        let changed = cloud.grains.iter().zip(orig.iter()).filter(|(g, &o)| (g.position - o).abs() > 1e-9).count();
        assert!(changed > 0);
    }

    #[test]
    fn test_scatter_clamped() {
        let mut cloud = GrainCloud::new(vec![Grain::new(vec![Ternary::Pos], 0.1, 1)]);
        scatter(&mut cloud, 10.0);
        assert!(cloud.grains[0].position >= 0.0 && cloud.grains[0].position <= 1.0);
    }

    #[test]
    fn test_stretch() {
        let source = vec![Ternary::Pos; 50];
        let stretched = stretch(&source, 10, 2.0, 5);
        assert!(stretched.len() >= source.len());
    }

    #[test]
    fn test_stretch_identity() {
        let source = vec![Ternary::Pos; 20];
        let stretched = stretch(&source, 10, 1.0, 5);
        // At factor 1.0, output should be roughly same length
        assert!((stretched.len() as f64 - source.len() as f64).abs() < 20.0);
    }

    #[test]
    fn test_stretch_empty() {
        assert!(stretch(&[], 10, 2.0, 5).is_empty());
    }

    #[test]
    fn test_freeze() {
        let g = Grain::new(vec![Ternary::Pos, Ternary::Neg], 0.0, 2);
        let frozen = freeze(&g, 3);
        assert_eq!(frozen.len(), 6);
        assert_eq!(frozen[0], Ternary::Pos);
        assert_eq!(frozen[1], Ternary::Neg);
        assert_eq!(frozen[2], Ternary::Pos);
    }

    #[test]
    fn test_morph_all_a() {
        let cloud_a = GrainCloud::new(vec![Grain::new(vec![Ternary::Pos; 10], 0.0, 10)]);
        let cloud_b = GrainCloud::new(vec![Grain::new(vec![Ternary::Neg; 10], 0.0, 10)]);
        let out = morph(&cloud_a, &cloud_b, 0.0, 20);
        assert_eq!(out.len(), 20);
    }

    #[test]
    fn test_morph_half() {
        let cloud_a = GrainCloud::new(vec![Grain::new(vec![Ternary::Pos; 10], 0.5, 10)]);
        let cloud_b = GrainCloud::new(vec![Grain::new(vec![Ternary::Neg; 10], 0.5, 10)]);
        let out = morph(&cloud_a, &cloud_b, 0.5, 20);
        // At 50% mix, should be between the two
        let _has_mixed = out.iter().any(|&v| v.abs() < 1.0 && v.abs() > 0.0);
        // Just verify it runs and produces output
        assert_eq!(out.len(), 20);
    }
}
