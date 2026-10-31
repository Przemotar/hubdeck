// Gaming B, a cyber rig: a dark carbon-fibre board behind the icons, one board through the fifteen
// windows, with glowing circuit traces running on a grid (straight runs, 45-degree bends, vias at
// their joints, a few chips), pulses of cyan and magenta light travelling along them, and HUD
// brackets glowing at every key's corners. A press: a power surge from the key, light racing out
// along the traces, the key's brackets flaring (`aux[0..3]`: x, y, start, 1 when on).
const G = 12.0;
fn sd_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return vec2<f32>(length(pa - ba * h), h);
}
// the traces: long runs on a 12 px grid, horizontal on some rows and vertical on some columns,
// each broken into stretches with gaps; .x the distance to the nearest, .y the position along it
// (px, for the pulses), .z its lane (speed and colour), .w 1 near a via (where a run starts or
// ends, or two cross)
fn runs(p: vec2<f32>) -> vec4<f32> {
    var best = vec4<f32>(1e3, 0.0, 0.0, 0.0);
    // horizontal runs: the nearest grid row
    let row = round(p.y / G - 0.5);
    for (var k = 0; k < 2; k++) {
        let r = row + f32(k);
        let y = (r + 0.5) * G;
        if (hash(vec2<f32>(r, 1.0)) < 0.62) { continue; }
        // stretches of 4-12 cells with a 1-3 cell gap, offset per row
        let len = 4.0 + floor(8.0 * hash(vec2<f32>(r, 2.0)));
        let gap = 1.0 + floor(2.0 * hash(vec2<f32>(r, 3.0)));
        let u = p.x / G + hash(vec2<f32>(r, 4.0)) * 9.0;
        let m = u - floor(u / (len + gap)) * (len + gap);
        if (m > len) { continue; }
        let d = abs(p.y - y);
        if (d < best.x) {
            let end = min(m, len - m) * G;
            best = vec4<f32>(d, p.x, hash(vec2<f32>(r, 5.0)), smoothstep(3.0, 1.0, end));
        }
    }
    // vertical runs: the nearest grid column
    let col = round(p.x / G - 0.5);
    for (var k = 0; k < 2; k++) {
        let cc = col + f32(k);
        let x = (cc + 0.5) * G;
        if (hash(vec2<f32>(cc, 11.0)) < 0.74) { continue; }
        let len = 3.0 + floor(6.0 * hash(vec2<f32>(cc, 12.0)));
        let gap = 2.0 + floor(3.0 * hash(vec2<f32>(cc, 13.0)));
        let u = p.y / G + hash(vec2<f32>(cc, 14.0)) * 9.0;
        let m = u - floor(u / (len + gap)) * (len + gap);
        if (m > len) { continue; }
        let d = abs(p.x - x);
        if (d < best.x) {
            let end = min(m, len - m) * G;
            best = vec4<f32>(d, p.y, hash(vec2<f32>(cc, 15.0)) + 0.37, smoothstep(3.0, 1.0, end));
        }
    }
    return best;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * f.speed;
    // carbon fibre: a fine twill weave, dark, a soft sheen across
    let w = vec2<f32>(p.x + p.y, p.x - p.y) / 3.0;
    let twill = 0.5 + 0.5 * sin(w.x * 3.14159) * sign(sin(w.y * 3.14159 * 0.5));
    var col = vec3<f32>(0.03, 0.035, 0.045) * (0.85 + 0.3 * twill);
    col += vec3<f32>(0.03, 0.04, 0.06) * smoothstep(0.6, 0.0, abs((p.x - p.y * 1.5) / 480.0 - 0.1));
    // the traces
    let tr = runs(p);
    let d = tr.x;
    let line = smoothstep(1.3, 0.4, d);
    // the copper under the light: dim teal, a via (a ring) at each run's ends
    col = mix(col, vec3<f32>(0.04, 0.14, 0.16), line * 0.8);
    let via = tr.w * smoothstep(0.6, 0.0, abs(d - 1.6)) ;
    col = mix(col, vec3<f32>(0.14, 0.34, 0.36), via);
    // pulses running along each run: each its own speed, direction and colour
    let lane = fract(tr.z);
    let dir = select(1.0, -1.0, lane > 0.5);
    let pulse = fract(tr.y / 90.0 * dir - t * (0.25 + 0.35 * lane) + lane * 7.0);
    let head = smoothstep(0.82, 0.99, pulse) * smoothstep(1.0, 0.99, pulse);
    let hue = select(vec3<f32>(0.1, 0.95, 1.0), vec3<f32>(1.0, 0.2, 0.85), lane > 0.7);
    col += hue * head * line * 1.2;
    col += hue * head * exp(-d / 3.0) * 0.25;
    // a press: a surge racing out along the traces
    var surge = 0.0;
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 1.5) { continue; }
        let dist = abs(p.x - b.x) + abs(p.y - b.y);
        surge = max(surge, exp(-abs(dist - age * 300.0) / 10.0) * (1.0 - age / 1.5));
    }
    col += vec3<f32>(0.6, 1.0, 1.0) * surge * line * 1.4 + vec3<f32>(0.2, 0.6, 0.7) * surge * exp(-d / 4.0) * 0.4;
    // HUD brackets at every key's corners, brighter on a press of that key
    let k = key_at(p);
    if (k >= 0) {
        let r = keys[k];
        let q = min(p - r.xy, r.xy + r.zw - p);
        let corner = min(q.x, q.y) < 2.2 && max(q.x, q.y) < 11.0 && min(q.x, q.y) > 0.6;
        var flare = 0.0;
        for (var i = 0; i < 3; i++) {
            let b = f.aux[i];
            if (b.w < 0.5) { continue; }
            if (key_at(b.xy) == k) { flare = max(flare, exp(-(f.time - b.z) * 2.5)); }
        }
        if (corner) { col = mix(col, vec3<f32>(0.2, 0.95, 1.0), 0.55 + 0.45 * flare); }
    }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
