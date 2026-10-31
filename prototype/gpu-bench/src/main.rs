//! The shader comparison of the living deck's stage 1 (docs/LIVING-DECK.md; owner, 2026-10-26:
//! "test tych shaderów na gpu żeby tworzyć super efekty"): the same ripple the engine draws on the
//! CPU (`canvas.rs` `Ripple`: a wave field at half the panel's size, 12 steps a frame, the picture
//! bent and lit by its slope), and a heavier one only a GPU could afford in the frame (the field at
//! the panel's full size, 48 steps, caustics from its curvature), each on the CPU and as WGSL
//! compute shaders, with the whole round trip the engine would pay: the panel picture up to the
//! GPU, the steps, the picture back. Prints the time a frame (median, 95th percentile), the GPU's
//! start-up time and the largest difference between the CPU's and the GPU's picture.
//!
//!   cargo run --release --manifest-path prototype/gpu-bench/Cargo.toml

use std::time::Instant;
use wgpu::util::DeviceExt;

const PW: usize = 480;
const PH: usize = 272;
const FRAMES: usize = 240;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    w: u32,
    h: u32,
    pw: u32,
    ph: u32,
    scale: f32,
    strength: f32,
    damp: f32,
    caustic: f32,
    tint: [f32; 4],
}

#[derive(Clone, Copy)]
struct Variant {
    name: &'static str,
    scale: usize,
    steps: usize,
    caustic: f32,
}

const VARIANTS: [Variant; 2] = [
    Variant { name: "engine's ripple (half-size field, 12 steps)", scale: 2, steps: 12, caustic: 0.0 },
    Variant { name: "heavy ripple (full-size field, 48 steps, caustics)", scale: 1, steps: 48, caustic: 1.0 },
];

/// A sample panel: dark keys with light marks, so bending and light show.
fn sample_panel() -> Vec<u32> {
    (0..PW * PH)
        .map(|i| {
            let (x, y) = (i % PW, i / PW);
            let key = (x % 100) < 79 && (y % 99) < 75;
            let mark = key && ((x % 100) as i32 - 39).abs() < 12 && ((y % 99) as i32 - 30).abs() < 12;
            let (r, g, b) = if mark { (121, 236, 198) } else if key { (27, 26, 32) } else { (0, 0, 0) };
            u32::from_le_bytes([r, g, b, 255])
        })
        .collect()
}

/// The drops of a press at frame `n` (the same for both ways).
fn drops(n: usize) -> Option<(f32, f32)> {
    match n {
        0 => Some((240.0, 136.0)),
        80 => Some((90.0, 60.0)),
        160 => Some((400.0, 220.0)),
        _ => None,
    }
}

fn press(field: &mut [f32], w: usize, h: usize, scale: usize, at: (f32, f32), strength: f32) {
    let (cx, cy) = (at.0 / scale as f32, at.1 / scale as f32);
    let r = 7.0 * 2.0 / scale as f32;
    for y in (cy - r) as i32..=(cy + r) as i32 {
        for x in (cx - r) as i32..=(cx + r) as i32 {
            if x < 1 || y < 1 || x >= w as i32 - 1 || y >= h as i32 - 1 {
                continue;
            }
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / r;
            if d < 1.0 {
                field[y as usize * w + x as usize] -= 14.0 * (0.4 + 0.6 * strength) * (0.5 + 0.5 * (d * std::f32::consts::PI).cos());
            }
        }
    }
}

// --------------------------------------------------------------------------------------------
// The CPU, as the engine does it

struct Cpu {
    a: Vec<f32>,
    b: Vec<f32>,
    w: usize,
    h: usize,
    v: Variant,
}

impl Cpu {
    fn new(v: Variant) -> Self {
        let (w, h) = (PW / v.scale, PH / v.scale);
        Self { a: vec![0.0; w * h], b: vec![0.0; w * h], w, h, v }
    }

    fn slope(&self, x: usize, y: usize) -> (f32, f32) {
        let s = self.v.scale as f32;
        let (fx, fy) = ((x as f32 / s).clamp(1.0, (self.w - 3) as f32), (y as f32 / s).clamp(1.0, (self.h - 3) as f32));
        let (ix, iy) = (fx as usize, fy as usize);
        let (u, v) = (fx - ix as f32, fy - iy as f32);
        let w = self.w;
        let g = |x: usize, y: usize| {
            let i = y * w + x;
            (self.a[i + 1] - self.a[i - 1], self.a[i + w] - self.a[i - w])
        };
        let (a, b, c, d) = (g(ix, iy), g(ix + 1, iy), g(ix, iy + 1), g(ix + 1, iy + 1));
        let lerp = |p: f32, q: f32, t: f32| p + (q - p) * t;
        (lerp(lerp(a.0, b.0, u), lerp(c.0, d.0, u), v), lerp(lerp(a.1, b.1, u), lerp(c.1, d.1, u), v))
    }

    fn curvature(&self, x: usize, y: usize) -> f32 {
        let s = self.v.scale;
        let (fx, fy) = ((x / s).clamp(1, self.w - 2), (y / s).clamp(1, self.h - 2));
        let i = fy * self.w + fx;
        self.a[i - 1] + self.a[i + 1] + self.a[i - self.w] + self.a[i + self.w] - 4.0 * self.a[i]
    }

    fn frame(&mut self, n: usize, src: &[u32], out: &mut [u32], strength: f32, tint: [f32; 3]) {
        if let Some(at) = drops(n) {
            press(&mut self.a, self.w, self.h, self.v.scale, at, strength);
        }
        for _ in 0..self.v.steps {
            for y in 1..self.h - 1 {
                for x in 1..self.w - 1 {
                    let i = y * self.w + x;
                    let v = (self.a[i - 1] + self.a[i + 1] + self.a[i - self.w] + self.a[i + self.w]) * 0.5 - self.b[i];
                    self.b[i] = v * 0.991;
                }
            }
            std::mem::swap(&mut self.a, &mut self.b);
        }
        for y in 0..PH {
            for x in 0..PW {
                let i = y * PW + x;
                let (dx, dy) = self.slope(x, y);
                let g = (dx * dx + dy * dy).sqrt();
                if g < 0.03 && self.v.caustic == 0.0 {
                    out[i] = src[i];
                    continue;
                }
                let (sx, sy) = ((x as f32 + dx * 4.5).clamp(0.0, (PW - 1) as f32) as usize, (y as f32 + dy * 4.5).clamp(0.0, (PH - 1) as f32) as usize);
                let p = src[sy * PW + sx].to_le_bytes();
                let mut c = [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0];
                let facing = (-dx * 0.55 - dy * 0.83).max(0.0);
                let band = (g * 0.32).min(1.0);
                let white = ((facing * 0.6) - 0.15).clamp(0.0, 1.0);
                let caustic = (-self.curvature(x, y) * 0.6).max(0.0).min(1.0) * self.v.caustic;
                for k in 0..3 {
                    c[k] = (c[k] + tint[k] * band * 0.9 * (0.5 + 0.5 * strength) + white + caustic * 0.8).min(1.0);
                }
                out[i] = u32::from_le_bytes([(c[0] * 255.0 + 0.5) as u8, (c[1] * 255.0 + 0.5) as u8, (c[2] * 255.0 + 0.5) as u8, 255]);
            }
        }
    }
}

// --------------------------------------------------------------------------------------------
// The GPU, as WGSL compute shaders

const WAVE: &str = r#"
struct P { w: u32, h: u32, pw: u32, ph: u32, scale: f32, strength: f32, damp: f32, caustic: f32, tint: vec4<f32> }
@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var<storage, read> a: array<f32>;
@group(0) @binding(2) var<storage, read_write> b: array<f32>;

@compute @workgroup_size(16, 16)
fn wave(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = id.x;
    let y = id.y;
    if (x < 1u || y < 1u || x >= p.w - 1u || y >= p.h - 1u) { return; }
    let i = y * p.w + x;
    let v = (a[i - 1u] + a[i + 1u] + a[i - p.w] + a[i + p.w]) * 0.5 - b[i];
    b[i] = v * p.damp;
}
"#;

const SHADE: &str = r#"
struct P { w: u32, h: u32, pw: u32, ph: u32, scale: f32, strength: f32, damp: f32, caustic: f32, tint: vec4<f32> }
@group(0) @binding(0) var<uniform> q: P;
@group(0) @binding(1) var<storage, read> f: array<f32>;
@group(0) @binding(2) var<storage, read> src: array<u32>;
@group(0) @binding(3) var<storage, read_write> dst: array<u32>;

fn grad(x: u32, y: u32) -> vec2<f32> {
    let i = y * q.w + x;
    return vec2<f32>(f[i + 1u] - f[i - 1u], f[i + q.w] - f[i - q.w]);
}

@compute @workgroup_size(16, 16)
fn shade(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= q.pw || id.y >= q.ph) { return; }
    let i = id.y * q.pw + id.x;
    let fx = clamp(f32(id.x) / q.scale, 1.0, f32(q.w - 3u));
    let fy = clamp(f32(id.y) / q.scale, 1.0, f32(q.h - 3u));
    let ix = u32(fx);
    let iy = u32(fy);
    let u = fx - f32(ix);
    let v = fy - f32(iy);
    let s = mix(mix(grad(ix, iy), grad(ix + 1u, iy), u), mix(grad(ix, iy + 1u), grad(ix + 1u, iy + 1u), u), v);
    let g = length(s);
    if (g < 0.03 && q.caustic == 0.0) { dst[i] = src[i]; return; }
    let sx = u32(clamp(f32(id.x) + s.x * 4.5, 0.0, f32(q.pw - 1u)));
    let sy = u32(clamp(f32(id.y) + s.y * 4.5, 0.0, f32(q.ph - 1u)));
    var c = unpack4x8unorm(src[sy * q.pw + sx]);
    let facing = max(-s.x * 0.55 - s.y * 0.83, 0.0);
    let band = min(g * 0.32, 1.0);
    let white = clamp(facing * 0.6 - 0.15, 0.0, 1.0);
    let cx = clamp(id.x / u32(q.scale), 1u, q.w - 2u);
    let cy = clamp(id.y / u32(q.scale), 1u, q.h - 2u);
    let k = cy * q.w + cx;
    let curv = f[k - 1u] + f[k + 1u] + f[k - q.w] + f[k + q.w] - 4.0 * f[k];
    let caustic = min(max(-curv * 0.6, 0.0), 1.0) * q.caustic;
    let lit = min(c.rgb + q.tint.rgb * band * 0.9 * (0.5 + 0.5 * q.strength) + vec3<f32>(white + caustic * 0.8), vec3<f32>(1.0));
    dst[i] = pack4x8unorm(vec4<f32>(lit, 1.0));
}
"#;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    wave: wgpu::ComputePipeline,
    shade: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    fa: wgpu::Buffer,
    fb: wgpu::Buffer,
    src: wgpu::Buffer,
    dst: wgpu::Buffer,
    back: wgpu::Buffer,
    ab: wgpu::BindGroup,
    ba: wgpu::BindGroup,
    shade_a: wgpu::BindGroup,
    shade_b: wgpu::BindGroup,
    field: Vec<f32>,
    w: usize,
    h: usize,
    v: Variant,
    /// Which buffer holds the newest field (false = `fa`).
    in_b: bool,
}

impl Gpu {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, v: Variant, strength: f32, tint: [f32; 3]) -> Self {
        let (w, h) = (PW / v.scale, PH / v.scale);
        let pipeline = |entry: &str, code: &str| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(entry), source: wgpu::ShaderSource::Wgsl(code.into()) });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some(entry), layout: None, module: &module, entry_point: Some(entry), compilation_options: Default::default(), cache: None })
        };
        let wave = pipeline("wave", WAVE);
        let shade = pipeline("shade", SHADE);
        let params = Params { w: w as u32, h: h as u32, pw: PW as u32, ph: PH as u32, scale: v.scale as f32, strength, damp: 0.991, caustic: v.caustic, tint: [tint[0], tint[1], tint[2], 0.0] };
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("params"), contents: bytemuck::bytes_of(&params), usage: wgpu::BufferUsages::UNIFORM });
        let storage = |label: &str, bytes: usize, extra: wgpu::BufferUsages| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: bytes as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | extra, mapped_at_creation: false });
        let fa = storage("field a", w * h * 4, wgpu::BufferUsages::COPY_SRC);
        let fb = storage("field b", w * h * 4, wgpu::BufferUsages::COPY_SRC);
        let src = storage("panel in", PW * PH * 4, wgpu::BufferUsages::empty());
        let dst = storage("panel out", PW * PH * 4, wgpu::BufferUsages::COPY_SRC);
        let back = device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: (PW * PH * 4) as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let group = |layout: &wgpu::BindGroupLayout, bufs: &[&wgpu::Buffer]| {
            let entries: Vec<wgpu::BindGroupEntry> = bufs.iter().enumerate().map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32, resource: b.as_entire_binding() }).collect();
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries })
        };
        let wl = wave.get_bind_group_layout(0);
        let sl = shade.get_bind_group_layout(0);
        let ab = group(&wl, &[&params, &fa, &fb]);
        let ba = group(&wl, &[&params, &fb, &fa]);
        let shade_a = group(&sl, &[&params, &fa, &src, &dst]);
        let shade_b = group(&sl, &[&params, &fb, &src, &dst]);
        let _ = queue;
        Self { device: device.clone(), queue: queue.clone(), wave, shade, params, fa, fb, src, dst, back, ab, ba, shade_a, shade_b, field: vec![0.0; w * h], w, h, v, in_b: false }
    }

    fn frame(&mut self, n: usize, src: &[u32], out: &mut [u32], strength: f32) {
        // a press: the newest field comes back, gets its drop, goes up again (as the engine would
        // on a press; between presses the field stays on the GPU)
        if let Some(at) = drops(n) {
            let newest = if self.in_b { &self.fb } else { &self.fa };
            self.field = read_back(&self.device, &self.queue, newest, self.w * self.h * 4).chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            press(&mut self.field, self.w, self.h, self.v.scale, at, strength);
            self.queue.write_buffer(newest, 0, bytemuck::cast_slice(&self.field));
        }
        self.queue.write_buffer(&self.src, 0, bytemuck::cast_slice(src));
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.wave);
            for _ in 0..self.v.steps {
                // a -> b when the newest is in a (b holds the one before, as the CPU's swap)
                pass.set_bind_group(0, if self.in_b { &self.ba } else { &self.ab }, &[]);
                pass.dispatch_workgroups((self.w as u32).div_ceil(16), (self.h as u32).div_ceil(16), 1);
                self.in_b = !self.in_b;
            }
            pass.set_pipeline(&self.shade);
            pass.set_bind_group(0, if self.in_b { &self.shade_b } else { &self.shade_a }, &[]);
            pass.dispatch_workgroups((PW as u32).div_ceil(16), (PH as u32).div_ceil(16), 1);
        }
        enc.copy_buffer_to_buffer(&self.dst, 0, &self.back, 0, (PW * PH * 4) as u64);
        self.queue.submit([enc.finish()]);
        let slice = self.back.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        self.device.poll(wgpu::PollType::Wait).expect("poll");
        out.copy_from_slice(bytemuck::cast_slice(&slice.get_mapped_range()));
        self.back.unmap();
        let _ = &self.params;
    }
}

fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, buf: &wgpu::Buffer, bytes: usize) -> Vec<u8> {
    let back = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_buffer_to_buffer(buf, 0, &back, 0, bytes as u64);
    queue.submit([enc.finish()]);
    let slice = back.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::Wait).expect("poll");
    let out = slice.get_mapped_range().to_vec();
    back.unmap();
    out
}

fn stats(mut ms: Vec<f64>) -> (f64, f64) {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (ms[ms.len() / 2], ms[ms.len() * 95 / 100])
}

fn main() {
    let strength = 0.6;
    let tint = [0.56, 0.85, 1.0];
    let panel = sample_panel();

    let start = Instant::now();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() })).expect("a GPU");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
    let info = adapter.get_info();
    println!("GPU: {} ({:?}, {:?}); start-up {:.0} ms", info.name, info.backend, info.device_type, start.elapsed().as_secs_f64() * 1000.0);

    // `paced <variant 0|1> <gpu|cpu>`: 16 frames a second for 30 s, as the engine would run it (for
    // power and clocks read beside it with nvidia-smi)
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("paced") {
        let v = VARIANTS[args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0usize).min(1)];
        let on_gpu = args.get(3).map(String::as_str) != Some("cpu");
        let mut gpu = Gpu::new(&device, &queue, v, strength, tint);
        let mut cpu = Cpu::new(v);
        let mut out = vec![0u32; PW * PH];
        let begin = Instant::now();
        let mut n = 0;
        while begin.elapsed().as_secs() < 30 {
            let due = begin + std::time::Duration::from_micros(62_500 * n as u64);
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            if on_gpu { gpu.frame(n % FRAMES, &panel, &mut out, strength) } else { cpu.frame(n % FRAMES, &panel, &mut out, strength, tint) }
            n += 1;
        }
        println!("paced {} on the {}: {n} frames in 30 s", v.name, if on_gpu { "GPU" } else { "CPU" });
        return;
    }

    for v in VARIANTS {
        let mut cpu = Cpu::new(v);
        let mut out_cpu = vec![0u32; PW * PH];
        let mut cpu_ms = Vec::new();
        let made = Instant::now();
        let mut gpu = Gpu::new(&device, &queue, v, strength, tint);
        let gpu_setup = made.elapsed().as_secs_f64() * 1000.0;
        let mut out_gpu = vec![0u32; PW * PH];
        let mut gpu_ms = Vec::new();
        let mut worst = 0u8;
        for n in 0..FRAMES {
            let t = Instant::now();
            cpu.frame(n, &panel, &mut out_cpu, strength, tint);
            cpu_ms.push(t.elapsed().as_secs_f64() * 1000.0);
            let t = Instant::now();
            gpu.frame(n, &panel, &mut out_gpu, strength);
            gpu_ms.push(t.elapsed().as_secs_f64() * 1000.0);
            if n % 20 == 10 {
                let diff = out_cpu.iter().zip(&out_gpu).flat_map(|(a, b)| a.to_le_bytes().into_iter().zip(b.to_le_bytes()).map(|(x, y)| x.abs_diff(y))).max().unwrap_or(0);
                worst = worst.max(diff);
            }
        }
        let (cm, c95) = stats(cpu_ms);
        let (gm, g95) = stats(gpu_ms);
        println!("{}:", v.name);
        println!("  CPU  {cm:6.2} ms a frame (95th {c95:6.2})  = {:5.1} % of a core at 16 a second", cm * 1.6);
        println!("  GPU  {gm:6.2} ms a frame (95th {g95:6.2}) with the picture up and back; pipelines made in {gpu_setup:.0} ms");
        println!("  largest difference CPU vs GPU in a colour channel: {worst} of 255");
    }
}
