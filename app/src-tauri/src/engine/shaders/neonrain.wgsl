// B8: neon rain: glowing streaks falling across the deck behind the icons in neon colours (the tint
// and its neighbours on the colour wheel), each landing on a key's floor in a small splash of light,
// the floor of each key wet and catching the colours, a soft haze of the city's light behind.
fn hue_shift(c: vec3<f32>, a: f32) -> vec3<f32> {
    // rotate a colour round the grey axis
    let k = vec3<f32>(0.57735);
    let ca = cos(a);
    return c * ca + cross(k, c) * sin(a) + k * dot(k, c) * (1.0 - ca);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time;
    // the city's haze: a slow glow from below in the neon colours
    let haze = hue_shift(f.tint.rgb, sin(p.x * 0.01 + t * 0.1) * 0.8) * (p.y / size.y) * 0.16;
    c = max(c, vec3<f32>(0.11)) + haze;
    // the rain: columns of drops, each column its own speed, length and colour
    let slant = 0.18 + f.wind * 0.5;
    let q = vec2<f32>(p.x + p.y * slant, p.y);
    let col = floor(q.x / 6.0);
    let r = hash(vec2<f32>(col, 3.0));
    if (r > 1.0 - 0.45 * clamp(f.density, 0.25, 2.0)) {
        let speed = 160.0 + 120.0 * hash(vec2<f32>(col, 4.0));
        let len = 14.0 + 22.0 * hash(vec2<f32>(col, 5.0));
        let period = size.y + len + 40.0;
        let head = fract((t * speed + hash(vec2<f32>(col, 6.0)) * period) / period) * period - len;
        let along = (q.y - head) / len;
        let x = col * 6.0 + 3.0;
        let d = abs(q.x - x);
        if (along > -1.0 && along < 0.05) {
            let fade = smoothstep(-1.0, 0.0, along);
            let tint = hue_shift(f.tint.rgb, (hash(vec2<f32>(col, 7.0)) - 0.5) * 1.6);
            c += tint * fade * (smoothstep(1.1, 0.0, d) * 0.9 + exp(-d * d / 6.0) * 0.25) * (0.5 + 0.6 * f.strength);
        }
    }
    // each key's floor: wet, a reflection of the haze, and splashes as drops land
    let k = key_at(p);
    if (k >= 0) {
        let rk = keys[k];
        let floor_y = rk.y + rk.w - 4.0;
        let wet = smoothstep(rk.y + rk.w - 12.0, floor_y, p.y);
        c += hue_shift(f.tint.rgb, (p.x - rk.x) * 0.02) * wet * 0.18 * f.strength;
        for (var i = 0; i < 3; i++) {
            let fi = f32(i);
            let life = 0.45;
            let cyc = t / life + hash(vec2<f32>(f32(k), fi)) * 7.0;
            let n = floor(cyc);
            let age = fract(cyc);
            let sx = rk.x + 8.0 + (rk.z - 16.0) * hash(vec2<f32>(f32(k) + n, fi + 9.0));
            let ring = abs(length((p - vec2<f32>(sx, floor_y)) * vec2<f32>(1.0, 3.2)) - age * 9.0);
            c += hue_shift(f.tint.rgb, fi) * smoothstep(1.2, 0.0, ring) * (1.0 - age) * 0.6 * f.strength * step(p.y, floor_y + 2.0);
        }
    }
    put(id.xy, c);
}
