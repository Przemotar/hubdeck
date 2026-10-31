// K1: each key a thick glass block, like the glass bricks of a wall. Under the icons, what lies
// behind (the keys' own backgrounds, the sky, water, the cover) is seen through the block: bent
// toward the middle at the wide bevel, rippled by the block's pressed pattern (soft waves, as glass
// bricks have), colours parting at the rim, light scattered inside the glass so even a dark key
// glows faintly toward the lit side. Over the icons: the bevel bright toward the deck's light
// (`aux[15]`, the lamp's when it is on) and dark away from it, a thin line of light caught inside
// the far edge, a hot spot, and a reflection sliding across, each block at its own angle.
const BEVEL: f32 = 12.0;
fn slope(p: vec2<f32>, r: vec4<f32>) -> vec2<f32> {
    let c = r.xy + r.zw * 0.5;
    let q = (p - c) / (r.zw * 0.5);
    let dx = min(p.x - r.x, r.x + r.z - p.x);
    let dy = min(p.y - r.y, r.y + r.w - p.y);
    var s = vec2<f32>(0.0);
    s.x = sign(q.x) * pow(1.0 - clamp(dx / BEVEL, 0.0, 1.0), 2.0);
    s.y = sign(q.y) * pow(1.0 - clamp(dy / BEVEL, 0.0, 1.0), 2.0);
    // the pressed pattern: soft waves across the block, the same on every block, as moulded glass
    let local = p - r.xy;
    s += vec2<f32>(sin(local.x * 0.42) * 0.10, sin(local.y * 0.42) * 0.10);
    s += q * 0.10;
    return s;
}
fn light_dir(r: vec4<f32>) -> vec2<f32> {
    if (f.aux[15].w > 0.0) { return normalize(f.aux[15].xy - (r.xy + r.zw * 0.5) + vec2<f32>(0.001)); }
    return normalize(vec2<f32>(-0.6, -0.8));
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let k = key_at(p);
    if (k < 0) { put(id.xy, px(i32(id.x), i32(id.y)).rgb); return; }
    let r = keys[k];
    let s = slope(p, r);
    let mid = r.xy + r.zw * 0.5;
    let bend = 10.0 * f.strength;
    let base = mid + (p - mid) * (1.0 - 0.12 * f.strength);
    let o = -s * bend;
    let cr = px_smooth(base + o * 1.10).r;
    let cg = px_smooth(base + o).g;
    let cb = px_smooth(base + o * 0.90).b;
    var c = vec3<f32>(cr, cg, cb);
    // the glass body: green-blue, thicker at the edges
    let thick = clamp(length(s), 0.0, 1.0);
    c = mix(c, c * vec3<f32>(0.80, 0.95, 0.98) + vec3<f32>(0.0, 0.03, 0.035), (0.25 + 0.6 * thick) * f.strength);
    // light scattered inside the block, brighter toward the lit side
    let ld = light_dir(r);
    let toward = clamp(dot((p - mid) / (r.zw * 0.5), ld) * 0.5 + 0.5, 0.0, 1.0);
    c += vec3<f32>(0.55, 0.70, 0.78) * (0.035 + 0.075 * toward * toward) * f.strength;
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = key_at(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let inner = key_inside(p, r);
    let ld = light_dir(r);
    let mid = r.xy + r.zw * 0.5;
    let k_s = f.strength;
    // the bevel's own slope (without the pattern), so the rim reads cleanly
    let q = (p - mid) / (r.zw * 0.5);
    let dx = min(p.x - r.x, r.x + r.z - p.x);
    let dy = min(p.y - r.y, r.y + r.w - p.y);
    let bx = sign(q.x) * pow(1.0 - clamp(dx / BEVEL, 0.0, 1.0), 2.0);
    let by = sign(q.y) * pow(1.0 - clamp(dy / BEVEL, 0.0, 1.0), 2.0);
    let bevel = vec2<f32>(bx, by);
    let rim = clamp(length(bevel), 0.0, 1.0);
    let facing = dot(normalize(bevel + vec2<f32>(0.0001)), ld);
    // lit toward the light, dark away from it; the outermost pixel a crisp bright edge
    c += vec3<f32>(0.88, 0.96, 1.0) * clamp(facing, 0.0, 1.0) * rim * 0.62 * k_s;
    c *= 1.0 - clamp(-facing, 0.0, 1.0) * rim * 0.38 * k_s;
    c += vec3<f32>(0.9, 0.97, 1.0) * smoothstep(1.6, 0.4, inner) * (0.10 + 0.25 * clamp(facing, 0.0, 1.0)) * k_s;
    // light caught inside the far edge: a thin bright line just in from the bevel
    let far = clamp(-facing, 0.0, 1.0);
    c += vec3<f32>(0.85, 0.97, 1.0) * exp(-pow(inner - BEVEL - 0.5, 2.0) * 0.45) * far * 0.30 * k_s;
    // a glint along the bevel nearest the light: long along the edge, thin across it
    let spot_at = mid + ld * (r.zw * 0.5 - vec2<f32>(4.0));
    let d = p - spot_at;
    let along = dot(d, vec2<f32>(-ld.y, ld.x));
    let across = dot(d, ld);
    c += vec3<f32>(1.0) * exp(-along * along / 90.0 - across * across / 3.0) * 0.55 * k_s;
    // a reflection band sliding across each block
    let local = (p - r.xy) / r.zw;
    let band = local.x * 0.8 + local.y * 0.55 - fract(f.time * 0.05 + f32(k) * 0.017) * 3.2 + 1.1;
    let sheen = exp(-band * band * 22.0) * 0.16 + exp(-pow(band - 0.2, 2.0) * 220.0) * 0.10;
    c += vec3<f32>(0.9, 0.96, 1.0) * sheen * k_s;
    put(id.xy, c);
}
