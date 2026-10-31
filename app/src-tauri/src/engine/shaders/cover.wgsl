// The cover as an aquarium of light (owner, 2026-10-26: "zrób też tę okładkę jako akwarium"): the
// now-playing cover (64x64, packed RGBA in the state; `aux[0].x` 1 when there is one) laid across
// the deck behind the icons, softened and flowing like light under water, its colours gathered
// into caustics, and bright fish of its own colours drifting through. Without a cover, the tint.
fn cover_px(x: i32, y: i32) -> vec3<f32> {
    let cx = clamp(x, 0, 63);
    let cy = clamp(y, 0, 63);
    return unpack4x8unorm(bitcast<u32>(now[u32(cy) * 64u + u32(cx)])).rgb;
}
fn cover_at(uv: vec2<f32>) -> vec3<f32> {
    if (f.aux[0].x < 0.5) {
        // nothing playing: a calm deep sea, its colours drifting slowly
        let n = vnoise(uv * 3.0 + vec2<f32>(f.time * 0.03, 0.0));
        return mix(vec3<f32>(0.05, 0.22, 0.38), vec3<f32>(0.20, 0.10, 0.42), n) * (0.75 + 0.5 * vnoise(uv * 7.0));
    }
    let p = uv * 64.0;
    let a = floor(p - 0.5);
    let t = p - 0.5 - a;
    let x = i32(a.x);
    let y = i32(a.y);
    return mix(mix(cover_px(x, y), cover_px(x + 1, y), t.x), mix(cover_px(x, y + 1), cover_px(x + 1, y + 1), t.x), t.y);
}
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += a * vnoise(q);
        q = q * 2.05 + vec2<f32>(2.3, 7.1);
        a *= 0.5;
    }
    return v;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let base = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time;
    // the cover fills the deck's width, centred, drifting slowly as if seen through moving water
    let uv0 = vec2<f32>(p.x / size.x, 0.5 + (p.y - size.y * 0.5) / size.x);
    let flow = vec2<f32>(fbm(uv0 * 3.0 + vec2<f32>(t * 0.05, 0.0)), fbm(uv0 * 3.0 + vec2<f32>(4.0, t * 0.04))) - 0.5;
    let uv = uv0 + flow * 0.08;
    // softened: four taps a little apart
    let o = 1.2 / 64.0;
    var col = (cover_at(uv + vec2<f32>(o, 0.0)) + cover_at(uv - vec2<f32>(o, 0.0)) + cover_at(uv + vec2<f32>(0.0, o)) + cover_at(uv - vec2<f32>(0.0, o))) * 0.25;
    // kept deep enough for the icons to read
    col = max(col * 0.42, vec3<f32>(0.09));
    // caustics in the cover's own colours
    let cp = p / 24.0;
    let w = vec2<f32>(vnoise(cp + vec2<f32>(t * 0.2, 0.0)), vnoise(cp + vec2<f32>(3.0, t * 0.16)));
    let cn = vnoise(cp * 1.6 + w * 2.0 + vec2<f32>(0.0, t * 0.08));
    let caustic = pow(1.0 - abs(cn * 2.0 - 1.0), 12.0);
    col += cover_at(uv * 0.9 + 0.05) * caustic * 0.30;
    // fish of light: soft bright shapes in the cover's colours, drifting through
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let speed = 12.0 + 14.0 * hash(vec2<f32>(fi, 1.0));
        let dir = select(-1.0, 1.0, hash(vec2<f32>(fi, 2.0)) > 0.5);
        let span = size.x + 80.0;
        let x = fract((hash(vec2<f32>(fi, 3.0)) * span + dir * speed * t) / span) * span - 40.0;
        let y = size.y * (0.15 + 0.7 * hash(vec2<f32>(fi, 4.0))) + sin(t * 0.6 + fi * 1.7) * 10.0;
        var q = p - vec2<f32>(x, y);
        q.x *= dir;
        let body = length(q / vec2<f32>(14.0, 5.0));
        let tail = length((q + vec2<f32>(13.0, 0.0)) / vec2<f32>(6.0, 4.5 + 2.0 * sin(t * 6.0 + fi)));
        let glow = exp(-body * body * 1.6) + exp(-tail * tail * 2.0) * 0.6;
        let hue = cover_at(vec2<f32>(hash(vec2<f32>(fi, 5.0)), hash(vec2<f32>(fi, 6.0))));
        col += (hue * 1.4 + vec3<f32>(0.10)) * glow * 0.9;
    }
    put(id.xy, mix(base, col, (0.85 * f.strength + 0.15) * (1.0 - keep_bg(base))));
}
