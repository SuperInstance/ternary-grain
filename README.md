# ternary-grain

**Granular synthesis of ternary streams.** Fragment, window, scatter, and recombine short slices of agent state sequences.

## Why This Exists

Granular synthesis revolutionized audio by decomposing sound into tiny overlapping fragments (grains) and recombining them in new ways. Instead of processing a continuous signal, you chop it into pieces, shape each piece with a window function, and layer them to create textures that were impossible with traditional synthesis.

The same principle applies to ternary agent streams. A long conversation trajectory contains patterns at multiple timescales. By slicing it into grains — short windows of ternary data — you can:

- **Scatter** grains across new positions to create variations on the original conversation
- **Stretch** conversations to arbitrary lengths without changing their character
- **Freeze** a moment and loop it indefinitely (the "stuck record" of a repetitive argument)
- **Morph** between two different conversation trajectories

This is the textural layer of the DJ metaphor. Where `ternary-crossfader` handles smooth transitions and `ternary-envelope` shapes dynamics, `ternary-grain` manipulates the raw fabric of the signal itself.

## The Physics Behind It

### Grains and Windowing

A grain is a short fragment of ternary data with a window function applied. The Hann window is used:

```
w(n) = 0.5 × (1 - cos(2πn/N))
```

This tapers the grain's edges to zero, preventing discontinuities when grains overlap. Without windowing, overlapping grains create clicks (in audio) or jarring state jumps (in agent dynamics).

The window has a specific property: when two Hann-windowed grains overlap at 50%, their sum is constant. This is the foundation of overlap-add synthesis and guarantees that time-stretching and pitch-shifting don't introduce artifacts.

### Grain Clouds

A `GrainCloud` is a collection of grains with shared parameters:

- **Density** — how many grains per unit length. High density = thick, lush texture. Low density = sparse, pointillistic.
- **Spread** — randomization of grain positions. Zero spread = all grains start at exactly the same source position. High spread = grains come from all over the source, creating a smeared, averaged version.

`from_source` creates a cloud by randomly sampling positions from the source signal, extracting grain-sized fragments, and optionally spreading them across the output range.

### Time Stretching

`stretch` changes the duration of a ternary signal without changing its content. It works by:

1. Extracting overlapping grains from the source at regular intervals
2. Spacing those grains further apart (for stretching) or closer together (for compressing)
3. Windowing each grain with Hann and overlap-adding into an output buffer
4. Normalizing by the overlap count at each position

At a stretch factor of 2.0, the output is twice as long. The content is the same — each state appears for twice as many samples. This is analogous to playing a record at half speed: same song, longer duration.

### Freeze

`freeze` loops a single grain for N repetitions. This creates a perfectly periodic pattern — the stuck-record effect. In agent dynamics, this models a conversation that's completely looped: the same arguments repeated verbatim.

The frozen output preserves the grain's data exactly, repeating it without windowing. If you want the frozen loop to be smooth, window the grain first.

### Morphing

`morph` crossfades between two grain clouds. At mix=0.0 you hear only cloud A. At mix=1.0 only cloud B. At mix=0.5, both are blended equally. This is the granular equivalent of `ternary-crossfader`'s linear crossfade, but operating at the grain level rather than the sample level.

### Connection to Fibonacci and Period 8

The natural grain size for ternary systems is related to the Pisano period 8. Grains of length 8 capture exactly one complete Fibonacci cycle in the ternary sequence `1, 1, -1, 0, -1, -1, 1, 0`. Grains of length 4 capture half a cycle. This is why the default `GrainParams` sets size=64 — it's 8 Fibonacci cycles, providing enough context for a meaningful fragment.

## Key Types and Functions

```rust
/// A ternary value.
pub enum Ternary { Neg, Zero, Pos }

/// A short windowed fragment of ternary data.
pub struct Grain {
    pub data: Vec<Ternary>,
    pub position: f64,   // start position in source (0.0..1.0)
    pub size: usize,
    pub pitch: f64,       // playback rate (1.0 = normal)
    pub amplitude: f64,   // 0.0..1.0
}

impl Grain {
    pub fn new(data: Vec<Ternary>, position: f64, size: usize) -> Self
    pub fn windowed(&self) -> Vec<f64>   // Hann-windowed output
    pub fn empty() -> Self
}

/// A collection of overlapping grains with shared parameters.
pub struct GrainCloud {
    pub grains: Vec<Grain>,
    pub density: f64,
    pub spread: f64,
}

impl GrainCloud {
    pub fn new(grains: Vec<Grain>) -> Self
    pub fn from_source(source: &[Ternary], grain_size: usize, count: usize, spread: f64) -> Self
    pub fn synthesize(&self, output_len: usize) -> Vec<f64>
}

/// Parameters controlling grain generation.
pub struct GrainParams {
    pub position: f64,
    pub size: usize,
    pub density: f64,
    pub spread: f64,
    pub pitch: f64,
}

/// Create grains from a source using parameters.
pub fn create_grains(source: &[Ternary], params: &GrainParams, count: usize) -> GrainCloud

/// Randomize grain positions within a cloud.
pub fn scatter(cloud: &mut GrainCloud, amount: f64)

/// Time-stretch by overlapping grains with adjustable overlap.
pub fn stretch(source: &[Ternary], grain_size: usize, stretch_factor: f64, overlap: usize) -> Vec<Ternary>

/// Loop a single grain for N repetitions.
pub fn freeze(grain: &Grain, repetitions: usize) -> Vec<Ternary>

/// Crossfade between two grain clouds.
pub fn morph(cloud_a: &GrainCloud, cloud_b: &GrainCloud, mix: f64, output_len: usize) -> Vec<f64>
```

## Usage

### Create and Synthesize a Grain Cloud

```rust
use ternary_grain::{Ternary, GrainCloud};

let source: Vec<Ternary> = (0..200).map(|i| match i % 3 {
    0 => Ternary::Pos,
    1 => Ternary::Neg,
    _ => Ternary::Zero,
}).collect();

// Create 20 grains of size 30, spread across the source
let cloud = GrainCloud::from_source(&source, 30, 20, 0.3);
let output = cloud.synthesize(500);
// 500 samples of granular texture derived from the source
```

### Time Stretching

```rust
use ternary_grain::stretch;

let signal: Vec<Ternary> = (0..100).map(|i| if i % 2 == 0 { Ternary::Pos } else { Ternary::Neg }).collect();

// Stretch to 2× length
let stretched = stretch(&signal, 20, 2.0, 10);
assert!(stretched.len() >= signal.len());

// Identity: factor 1.0 should be roughly the same length
let identity = stretch(&signal, 20, 1.0, 10);
```

### Freeze a Moment

```rust
use ternary_grain::{Grain, freeze, Ternary};

let grain = Grain::new(vec![Ternary::Pos, Ternary::Neg, Ternary::Zero], 0.0, 3);
let frozen = freeze(&grain, 5);
// [Pos, Neg, Zero, Pos, Neg, Zero, Pos, Neg, Zero, Pos, Neg, Zero, Pos, Neg, Zero]
assert_eq!(frozen.len(), 15);
```

### Scatter and Morph

```rust
use ternary_grain::{Ternary, GrainCloud, scatter, morph, create_grains, GrainParams};

let source_a: Vec<Ternary> = (0..100).map(|_| Ternary::Pos).collect();
let source_b: Vec<Ternary> = (0..100).map(|_| Ternary::Neg).collect();

let params = GrainParams { size: 20, spread: 0.3, ..Default::default() };
let mut cloud_a = create_grains(&source_a, &params, 10);
let cloud_b = create_grains(&source_b, &params, 10);

// Scatter cloud_a for variation
scatter(&mut cloud_a, 0.5);

// Morph between the two clouds
let blended = morph(&cloud_a, &cloud_b, 0.5, 200);
// Half agreement, half contrarian — averaged texture
```

## In the Ternary Fleet

This is the **textural processing** layer in the DJ metaphor product stack:

- `ternary-tenforward` — produces the raw agent trajectories that grains are cut from
- `ternary-sampler` — provides the sampling strategies for selecting grain source material
- `ternary-envelope` — grains use Hann windows (a specific envelope shape)
- `ternary-crossfader` — grain morphing uses crossfading internally
- **ternary-grain** — fragments and recombines at the micro level
- `ternary-rack** — grains and clouds can be rooms in the processing rack

## References

- Granular synthesis: Dennis Gabor (1946), Curtis Roads (1980s)
- Hann window: `w(n) = 0.5(1 - cos(2πn/N))` — raised cosine, perfect overlap-add at 50%
- Fibonacci period 8: natural grain size is one Pisano period for mod 3
- Overlap-add: the mathematical foundation that makes time-stretching artifact-free

## License

MIT
