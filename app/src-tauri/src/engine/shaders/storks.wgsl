// Poland's autumn morning: now and then a skein of storks crosses the deck in a V, flying south
// (right to left), behind the icons: white bodies and wings with black flight feathers, long necks
// stretched out, red beaks and legs trailing, slow wingbeats between glides. One crossing every
// ~40 s (density: more often), 14 s across.
fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    // distance to the segment ab, and where along it (0..1)
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / dot(ab, ab), 0.0, 1.0);
    return vec2<f32>(length(p - a - ab * t), t);
}
// one stork at q (its own space: flying to -x), its size s (wing span ~2.4 s), its wingbeat w
// (-1..1); returns the colour's alpha in .x and which part in .y (0 white, 1 black, 2 red)
fn stork(q: vec2<f32>, s: f32, w: f32) -> vec2<f32> {
    var best = vec2<f32>(1e3, 0.0);
    // body: a long oval
    let body = length(q / vec2<f32>(s * 0.42, s * 0.13)) - 1.0;
    best = vec2<f32>(body * s * 0.13, 0.0);
    // neck and head forward, the beak red
    let neck = seg(q, vec2<f32>(-s * 0.3, -s * 0.02), vec2<f32>(-s * 0.78, -s * 0.06)).x - s * 0.045;
    if (neck < best.x) { best = vec2<f32>(neck, 0.0); }
    let beak = seg(q, vec2<f32>(-s * 0.78, -s * 0.06), vec2<f32>(-s * 1.02, -s * 0.03)).x - s * 0.028;
    if (beak < best.x) { best = vec2<f32>(beak, 2.0); }
    // legs trailing behind, red
    let legs = seg(q, vec2<f32>(s * 0.35, s * 0.04), vec2<f32>(s * 0.95, s * 0.09)).x - s * 0.022;
    if (legs < best.x) { best = vec2<f32>(legs, 2.0); }
    // the wing seen from the side: from the shoulder up or down with the beat, broad, its outer
    // half (the flight feathers) black
    let tip = vec2<f32>(-s * 0.05, -s * 1.15 * w);
    let wing = seg(q, vec2<f32>(-s * 0.08, -s * 0.02), tip);
    let wd = wing.x - s * mix(0.17, 0.08, wing.y);
    if (wd < best.x) { best = vec2<f32>(wd, select(0.0, 1.0, wing.y > 0.5)); }
    // the black trailing edge along the whole wing
    let trail = seg(q, vec2<f32>(s * 0.06, -s * 0.0), tip + vec2<f32>(s * 0.12, 0.0));
    let td = trail.x - s * 0.05;
    if (td < best.x && wing.y > 0.15) { best = vec2<f32>(td, 1.0); }
    return best;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let every = 40.0 / max(f.density, 0.25);
    let crossing = floor(f.time / every);
    let u = (f.time - crossing * every) / 14.0;
    if (u > 1.0) { put(id.xy, c); return; }
    // the leader's path: right to left, a gentle rise and fall, its height new each crossing
    let lead = vec2<f32>(size.x + 160.0 - u * (size.x + 480.0), 50.0 + 70.0 * hash(vec2<f32>(crossing, 3.0)) + 10.0 * sin(u * 5.0));
    if (p.x < lead.x - 60.0) { put(id.xy, c); return; }
    for (var j = 0; j < 7; j++) {
        let fj = f32(j);
        let row = f32((j + 1) / 2);
        let side = select(1.0, -1.0, j % 2 == 1);
        let pos = lead + vec2<f32>(row * 46.0, row * 22.0 * side) + vec2<f32>(0.0, sin(f.time * 0.9 + fj) * 2.0);
        let q = p - pos;
        let s = 24.0;
        if (abs(q.x) > s * 1.3 || abs(q.y) > s * 1.3) { continue; }
        // beats for a while, then glides with the wings level
        let beating = smoothstep(-0.2, 0.3, sin(f.time * 0.45 + fj * 0.8));
        let w = mix(0.08, sin(f.time * 3.4 + fj * 0.7), beating);
        let st = stork(q, s, w);
        let a = smoothstep(0.6, -0.6, st.x) * min(f.strength / 0.6, 1.3);
        var col = vec3<f32>(0.96, 0.95, 0.92);
        if (st.y > 0.5) { col = vec3<f32>(0.05, 0.05, 0.06); }
        if (st.y > 1.5) { col = vec3<f32>(0.9, 0.22, 0.12); }
        // the low sun from the left warms their undersides a touch
        col *= vec3<f32>(1.0, 0.94, 0.84);
        c = mix(c, col, a);
    }
    put(id.xy, c);
}
