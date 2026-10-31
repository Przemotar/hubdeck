//! The living deck on the graphics card (owner, 2026-10-26: "test tych shaderów na gpu żeby
//! tworzyć super efekty"; "1 a": the GPU in the engine first). A shader layer is a small WGSL
//! program with fixed inputs, the same for every layer (Shadertoy's and ISF's idea), so new layers
//! are written as data and checked the same way:
//!
//! - binding 0, `Frame`: size, time, dt, tint, strength, speed, density, wind, the screen's "down",
//!   how many presses are new, a seed, and sixteen vec4s of the layer's own (`aux`: where the moon
//!   goes, a palette...);
//! - 1 `src`: the panel as drawn so far (RGBA8 packed in a u32 a pixel), 2 `dst`: what the layer
//!   makes of it (every pixel must be written);
//! - 3 `mask`: how much of each pixel is the keys' content (icon, label, picture), 0..255, four
//!   pixels a u32 (`cover(x, y)` reads it);
//! - 4 `keys`: the 15 key windows (x, y, w, h); 5 `press`: up to 16 new presses (x, y, strength,
//!   key) since the last step;
//! - 6 `now` and 7 `next`: the layer's own state (a wave field...), swapped after every `step`.
//!
//! Entry points: `step` (optional, run `steps` times a frame over the state), `under` and `over`
//! (optional, one thread a panel pixel). The bench (`prototype/gpu-bench`, 2026-10-26, RTX 3060 Ti):
//! the panel up, a heavy wave field and back in 1.5 ms where the CPU took 9.1.

use std::sync::OnceLock;

use super::render::{PANEL_H, PANEL_W};

const W: usize = PANEL_W as usize;
const H: usize = PANEL_H as usize;
const PRESSES: usize = 16;

/// The card, made once on first use (0.7-1 s); None when there is none (or it failed): GPU layers
/// then draw nothing and the window says so.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

pub fn get() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(|| {
        // a test copy can act as a PC without a card (the panel's "no graphics card" line)
        if std::env::var_os("HUBDECK_NO_GPU").is_some() {
            return None;
        }
        let started = std::time::Instant::now();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() })).ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let info = adapter.get_info();
        let name = format!("{} ({:?})", info.name, info.backend);
        log::info!("living deck: GPU {name} ready in {} ms", started.elapsed().as_millis());
        Some(Gpu { device, queue })
    })
    .as_ref()
}

/// What every shader layer gets each frame (WGSL `Frame`, 16-byte aligned).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Frame {
    size: [u32; 2],
    time: f32,
    dt: f32,
    tint: [f32; 4],
    strength: f32,
    speed: f32,
    density: f32,
    wind: f32,
    down: [f32; 2],
    presses: u32,
    seed: u32,
    aux: [[f32; 4]; 16],
}

/// The WGSL every layer starts with: the inputs and small helpers.
const PRELUDE: &str = r#"
struct Frame {
    size: vec2<u32>,
    time: f32,
    dt: f32,
    tint: vec4<f32>,
    strength: f32,
    speed: f32,
    density: f32,
    wind: f32,
    down: vec2<f32>,
    presses: u32,
    seed: u32,
    aux: array<vec4<f32>, 16>,
}
@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var<storage, read> src: array<u32>;
@group(0) @binding(2) var<storage, read_write> dst: array<u32>;
@group(0) @binding(3) var<storage, read> mask: array<u32>;
@group(0) @binding(4) var<storage, read> keys: array<vec4<f32>, 15>;
@group(0) @binding(5) var<storage, read> press: array<vec4<f32>, 16>;
@group(0) @binding(6) var<storage, read> now: array<f32>;
@group(0) @binding(7) var<storage, read_write> next: array<f32>;

fn px(x: i32, y: i32) -> vec4<f32> {
    let cx = clamp(x, 0, i32(f.size.x) - 1);
    let cy = clamp(y, 0, i32(f.size.y) - 1);
    return unpack4x8unorm(src[u32(cy) * f.size.x + u32(cx)]);
}
fn cover(x: i32, y: i32) -> f32 {
    let cx = clamp(x, 0, i32(f.size.x) - 1);
    let cy = clamp(y, 0, i32(f.size.y) - 1);
    let i = u32(cy) * f.size.x + u32(cx);
    return f32((mask[i / 4u] >> ((i % 4u) * 8u)) & 255u) / 255.0;
}
// The point of the 480x480 world square a panel pixel shows, for the deck's turn (`f.down`, the
// screen's "down" in panel coordinates): landscape shows y 104..376 of it, portrait x 104..376, and
// the centre 3x3 keys (100..380 both ways) are seen in both (owner, 2026-10-29: the deck can stand
// upright; `docs/LIVING-DECK.md` > The world square). A scene drawn in these coordinates reads
// upright in every turn.
fn world(p: vec2<f32>) -> vec2<f32> {
    let d = f.down;
    let r = vec2<f32>(d.y, -d.x);
    let s = vec2<f32>(f32(f.size.x), f32(f.size.y));
    let v = vec2<f32>(dot(p, r) - min(r.x, 0.0) * s.x - min(r.y, 0.0) * s.y, dot(p, d) - min(d.x, 0.0) * s.x - min(d.y, 0.0) * s.y);
    let seen = vec2<f32>(abs(r.x) * s.x + abs(r.y) * s.y, abs(d.x) * s.x + abs(d.y) * s.y);
    return v + (vec2<f32>(480.0) - seen) * 0.5;
}
// The part of the world square the panel shows, in its own upright coordinates (the upright copies
// of the scenes, owner 2026-10-29: "przerobić tak żeby też dobrze wyglądały w układzie pionowym"):
// its size, 480x272 landscape and 272x480 upright; a panel pixel in it (landscape: the pixel
// itself); back to the panel; a key window (panel x, y, w, h) in it.
fn view_size() -> vec2<f32> {
    return select(vec2<f32>(480.0, 272.0), vec2<f32>(272.0, 480.0), abs(f.down.x) > 0.5);
}
fn to_view(p: vec2<f32>) -> vec2<f32> {
    return world(p) - (vec2<f32>(480.0) - view_size()) * 0.5;
}
fn from_view(q: vec2<f32>) -> vec2<f32> {
    let d = f.down;
    let r = vec2<f32>(d.y, -d.x);
    let s = vec2<f32>(f32(f.size.x), f32(f.size.y));
    let a = q.x + min(r.x, 0.0) * s.x + min(r.y, 0.0) * s.y;
    let b = q.y + min(d.x, 0.0) * s.x + min(d.y, 0.0) * s.y;
    return a * r + b * d;
}
fn view_rect(r: vec4<f32>) -> vec4<f32> {
    let a = to_view(r.xy);
    let b = to_view(r.xy + r.zw);
    return vec4<f32>(min(a, b), abs(b - a));
}
// the same number for the same place every frame (where a star stands); `hash_t` for a new one
// each frame (grain). Session 33: one hash with the frame's seed in it made the stars wander.
fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}
fn hash_t(p: vec2<f32>) -> f32 {
    return hash(p + vec2<f32>(f32(f.seed % 7919u) * 1.37, f32(f.seed % 6007u) * 0.71));
}
// smooth value noise, 0..1
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let t = p - i;
    let u = t * t * (3.0 - 2.0 * t);
    return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x), mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}
// half a step of 8-bit colour of noise before the picture is stored: no bands in dark gradients;
// the same each frame, so a still pixel stays still (nothing new to send)
fn dither(c: vec3<f32>, p: vec2<u32>) -> vec3<f32> {
    let n = hash(vec2<f32>(p) * 1.618) - 0.5;
    return c + vec3<f32>(n / 255.0);
}
// how far p is inside a key window (negative outside), and which window holds p (-1: a frame)
fn key_inside(p: vec2<f32>, r: vec4<f32>) -> f32 {
    return min(min(p.x - r.x, r.x + r.z - p.x), min(p.y - r.y, r.y + r.w - p.y));
}
fn key_at(p: vec2<f32>) -> i32 {
    for (var k = 0; k < 15; k++) {
        if (key_inside(p, keys[k]) > 0.0) { return k; }
    }
    return -1;
}
// the picture so far between pixels (p in pixel units, centres at .5)
fn px_smooth(p: vec2<f32>) -> vec3<f32> {
    let a = floor(p - 0.5);
    let t = p - 0.5 - a;
    let x = i32(a.x);
    let y = i32(a.y);
    return mix(mix(px(x, y).rgb, px(x + 1, y).rgb, t.x), mix(px(x, y + 1).rgb, px(x + 1, y + 1).rgb, t.x), t.y);
}
// how much of a key's own background a scene behind the icons keeps: a key with its own lighter
// colour stays a key (owner, 2026-10-26: under the aurora "szare przyciski … robią się
// niewidoczne"); the default dark keys give way to the scene
fn keep_bg(base: vec3<f32>) -> f32 {
    return smoothstep(0.13, 0.26, dot(base, vec3<f32>(0.299, 0.587, 0.114))) * 0.8;
}
// the layer's own number i of the 64 in `aux`
fn aux_at(i: i32) -> f32 {
    return f.aux[i / 4][i % 4];
}
fn put(p: vec2<u32>, c: vec3<f32>) {
    dst[p.y * f.size.x + p.x] = pack4x8unorm(vec4<f32>(clamp(dither(c, p), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
"#;

/// A shader layer at work on the card: its programs, buffers and state.
pub struct ShaderLayer {
    gpu: &'static Gpu,
    step: Option<wgpu::ComputePipeline>,
    under: Option<wgpu::ComputePipeline>,
    over: Option<wgpu::ComputePipeline>,
    frame: wgpu::Buffer,
    src: wgpu::Buffer,
    dst: wgpu::Buffer,
    mask: wgpu::Buffer,
    keys: wgpu::Buffer,
    press: wgpu::Buffer,
    back: wgpu::Buffer,
    state_a: wgpu::Buffer,
    state_b: wgpu::Buffer,
    /// Bind groups with the state one way and the other (`now`/`next` swap after each step).
    groups: [wgpu::BindGroup; 2],
    flip: usize,
    /// Presses since the last step (x, y, strength, key).
    presses: Vec<[f32; 4]>,
    /// The state's size in cells a step runs over (x, y).
    state_size: (u32, u32),
    /// The layer's own inputs (`Frame::aux`), set by its effect before a step or a draw.
    pub aux: [[f32; 4]; 16],
    /// Its `mask` is the icons' cover without labels ([`super::canvas::Inputs::icons`]).
    pub icons_only: bool,
}

/// A layer's compiled program: its bind group layout and its `step`, `under`, `over` pipelines.
type Compiled = (wgpu::BindGroupLayout, Option<wgpu::ComputePipeline>, Option<wgpu::ComputePipeline>, Option<wgpu::ComputePipeline>);

/// The programs compiled so far, by name and code: a layer made again reuses its program.
fn compiled() -> &'static std::sync::Mutex<std::collections::HashMap<u64, Compiled>> {
    static COMPILED: OnceLock<std::sync::Mutex<std::collections::HashMap<u64, Compiled>>> = OnceLock::new();
    COMPILED.get_or_init(Default::default)
}

impl ShaderLayer {
    /// A layer from its WGSL body (after [`PRELUDE`]) and the size of its state, in f32 cells (two
    /// buffers of it); None without a card or when the program does not compile.
    pub fn new(name: &str, body: &str, state: (u32, u32)) -> Option<Self> {
        let gpu = get()?;
        let device = &gpu.device;
        let code = format!("{PRELUDE}\n{body}");
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, &code).hash(&mut h);
            h.finish()
        };
        // the lock is let go before compiling (a guard in the match would hold it into the insert)
        let kept = compiled().lock().unwrap_or_else(|e| e.into_inner()).get(&key).cloned();
        let (layout, step, under, over) = match kept {
            Some(c) => c,
            None => {
                let c = Self::compile(device, name, &code, body)?;
                compiled().lock().unwrap_or_else(|e| e.into_inner()).insert(key, c.clone());
                c
            }
        };
        Self::with(device, layout, step, under, over, state)
    }

    /// The layer's program compiled: its bind group layout and its `step`, `under` and `over`
    /// pipelines (a big scene took 1-2 s the first time; owner, 2026-10-30, "5. tak jak piszesz":
    /// compiled once, kept, and ahead of time by [`super::canvas::warm_up`]).
    fn compile(device: &wgpu::Device, name: &str, code: &str, body: &str) -> Option<Compiled> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(name), source: wgpu::ShaderSource::Wgsl(code.into()) });
        let storage = |read_only: bool| wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None };
        let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(name),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(1, storage(true)),
                entry(2, storage(false)),
                entry(3, storage(true)),
                entry(4, storage(true)),
                entry(5, storage(true)),
                entry(6, storage(true)),
                entry(7, storage(false)),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(name), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let has = |entry: &str| body.contains(&format!("fn {entry}("));
        let pipeline = |entry: &str| {
            has(entry).then(|| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
        };
        let (step, under, over) = (pipeline("step"), pipeline("under"), pipeline("over"));
        if let Some(e) = pollster::block_on(device.pop_error_scope()) {
            log::warn!("living deck: GPU layer {name} does not compile: {e}");
            return None;
        }
        Some((layout, step, under, over))
    }

    /// A layer of a compiled program: its own buffers and bind groups.
    fn with(
        device: &wgpu::Device,
        layout: wgpu::BindGroupLayout,
        step: Option<wgpu::ComputePipeline>,
        under: Option<wgpu::ComputePipeline>,
        over: Option<wgpu::ComputePipeline>,
        state: (u32, u32),
    ) -> Option<Self> {
        let gpu = get()?;
        let buf =
            |label: &str, size: usize, usage: wgpu::BufferUsages| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: size.max(16) as u64, usage, mapped_at_creation: false });
        let rw = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        let frame = buf("frame", std::mem::size_of::<Frame>(), wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let src = buf("src", W * H * 4, rw);
        let dst = buf("dst", W * H * 4, rw);
        let mask = buf("mask", W * H, rw);
        let keys = buf("keys", 15 * 16, rw);
        let press = buf("press", PRESSES * 16, rw);
        let cells = (state.0 * state.1) as usize * 4;
        let a = buf("state a", cells, rw);
        let b = buf("state b", cells, rw);
        let back = buf("readback", W * H * 4, wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        let group = |now: &wgpu::Buffer, next: &wgpu::Buffer| {
            let bufs = [&frame, &src, &dst, &mask, &keys, &press, now, next];
            let entries: Vec<wgpu::BindGroupEntry> = bufs.iter().enumerate().map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32, resource: b.as_entire_binding() }).collect();
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &layout, entries: &entries })
        };
        let groups = [group(&a, &b), group(&b, &a)];
        Some(Self {
            gpu,
            step,
            under,
            over,
            frame,
            src,
            dst,
            mask,
            keys,
            press,
            back,
            state_a: a,
            state_b: b,
            groups,
            flip: 0,
            presses: Vec::new(),
            state_size: state,
            aux: [[0.0; 4]; 16],
            icons_only: false,
        })
    }

    /// Puts data of its own in both state buffers (a picture as packed RGBA in f32 cells: the
    /// cover, the page before a page turn), from cell `at`.
    pub fn upload_state(&self, at: usize, cells: &[u32]) {
        let q = &self.gpu.queue;
        let bytes: &[u8] = bytemuck::cast_slice(cells);
        let size = (self.state_size.0 * self.state_size.1) as usize;
        if at + cells.len() <= size {
            for g in [&self.state_a, &self.state_b] {
                q.write_buffer(g, (at * 4) as u64, bytes);
            }
        }
    }

    pub fn press(&mut self, x: f32, y: f32, strength: f32, key: usize) {
        if self.presses.len() < PRESSES {
            self.presses.push([x, y, strength, key as f32]);
        }
    }

    fn frame(&self, input: &super::canvas::Inputs, presses: u32) -> Frame {
        Frame {
            size: [W as u32, H as u32],
            time: input.t,
            dt: input.dt,
            tint: [input.tint[0], input.tint[1], input.tint[2], 1.0],
            strength: input.strength,
            speed: input.speed,
            density: input.density,
            wind: input.wind,
            down: [input.down.0, input.down.1],
            presses,
            seed: (input.t * 1000.0) as u32,
            aux: self.aux,
        }
    }

    /// Runs the state `steps` times (new presses go in with the first); nothing without a `step`.
    fn write_keys(&self, input: &super::canvas::Inputs) {
        let mut keys = [[0f32; 4]; 15];
        for (k, p) in keys.iter_mut().zip(input.keys) {
            *k = [p.x, p.y, p.w, p.h];
        }
        self.gpu.queue.write_buffer(&self.keys, 0, bytemuck::cast_slice(&keys));
    }

    pub fn step(&mut self, input: &super::canvas::Inputs, steps: u32) {
        let Some(pipeline) = &self.step else { return };
        let q = &self.gpu.queue;
        let presses: Vec<[f32; 4]> = std::mem::take(&mut self.presses);
        self.write_keys(input);
        let mut pad = [[0f32; 4]; PRESSES];
        pad[..presses.len()].copy_from_slice(&presses);
        q.write_buffer(&self.press, 0, bytemuck::cast_slice(&pad));
        let mut enc = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        for n in 0..steps.max(1) {
            // the presses only in the first step
            let frame = self.frame(input, if n == 0 { presses.len() as u32 } else { 0 });
            // each step its own frame values: written through a staging copy in order
            let staging = wgpu::util::DeviceExt::create_buffer_init(
                &self.gpu.device,
                &wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::bytes_of(&frame), usage: wgpu::BufferUsages::COPY_SRC },
            );
            enc.copy_buffer_to_buffer(&staging, 0, &self.frame, 0, std::mem::size_of::<Frame>() as u64);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.groups[self.flip], &[]);
            pass.dispatch_workgroups(self.state_size.0.div_ceil(16), self.state_size.1.div_ceil(16), 1);
            drop(pass);
            self.flip ^= 1;
        }
        q.submit([enc.finish()]);
    }

    pub fn has_under(&self) -> bool {
        self.under.is_some()
    }

    pub fn has_over(&self) -> bool {
        self.over.is_some()
    }

    /// The program `draw` would run (`under` or `over`), if the layer has it.
    pub fn program(&self, over: bool) -> Option<&wgpu::ComputePipeline> {
        if over { self.over.as_ref() } else { self.under.as_ref() }
    }

    /// The panel through the layer's `under` or `over` program, in place (up to the card and back).
    pub fn draw(&mut self, input: &super::canvas::Inputs, panel: &mut [u8], over: bool) {
        let Some(pipeline) = (if over { &self.over } else { &self.under }) else { return };
        let q = &self.gpu.queue;
        q.write_buffer(&self.frame, 0, bytemuck::bytes_of(&self.frame(input, 0)));
        self.write_keys(input);
        // RGBA bytes are the u32s the shader reads (little-endian, as `unpack4x8unorm` wants)
        q.write_buffer(&self.src, 0, panel);
        let mask = if self.icons_only && input.icons.len() == W * H { input.icons } else { input.mask };
        if mask.len() == W * H {
            q.write_buffer(&self.mask, 0, mask);
        }
        let mut enc = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.groups[self.flip], &[]);
            pass.dispatch_workgroups((W as u32).div_ceil(16), (H as u32).div_ceil(16), 1);
        }
        enc.copy_buffer_to_buffer(&self.dst, 0, &self.back, 0, (W * H * 4) as u64);
        q.submit([enc.finish()]);
        let slice = self.back.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        if self.gpu.device.poll(wgpu::PollType::Wait).is_err() {
            return;
        }
        panel.copy_from_slice(&slice.get_mapped_range());
        self.back.unmap();
    }
}

/// Several layers drawn one after another on the card: the panel goes up once, each layer's
/// result is the next one's picture on the card, and it comes back once (each on its own went up
/// and back: ~1.1-1.5 ms a layer even for a light one, session 35's frame cost). Every layer must
/// have the program asked for ([`ShaderLayer::program`]). False when the card did not answer (the
/// panel is then as it was).
pub fn draw_chain(chain: &[(&ShaderLayer, &super::canvas::Inputs)], panel: &mut [u8], over: bool) -> bool {
    let Some((first, _)) = chain.first() else { return true };
    let gpu = first.gpu;
    let q = &gpu.queue;
    q.write_buffer(&first.src, 0, panel);
    for (layer, input) in chain {
        q.write_buffer(&layer.frame, 0, bytemuck::bytes_of(&layer.frame(input, 0)));
        layer.write_keys(input);
        let mask = if layer.icons_only && input.icons.len() == W * H { input.icons } else { input.mask };
        if mask.len() == W * H {
            q.write_buffer(&layer.mask, 0, mask);
        }
    }
    let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    for (i, (layer, _)) in chain.iter().enumerate() {
        let Some(pipeline) = layer.program(over) else { return false };
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &layer.groups[layer.flip], &[]);
            pass.dispatch_workgroups((W as u32).div_ceil(16), (H as u32).div_ceil(16), 1);
        }
        let to = match chain.get(i + 1) {
            Some((next, _)) => &next.src,
            None => &layer.back,
        };
        enc.copy_buffer_to_buffer(&layer.dst, 0, to, 0, (W * H * 4) as u64);
    }
    q.submit([enc.finish()]);
    let last = chain[chain.len() - 1].0;
    let slice = last.back.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    if gpu.device.poll(wgpu::PollType::Wait).is_err() {
        return false;
    }
    panel.copy_from_slice(&slice.get_mapped_range());
    last.back.unmap();
    true
}
