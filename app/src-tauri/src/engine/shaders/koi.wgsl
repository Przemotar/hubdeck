// Japan: a koi pond seen from above behind the icons, one pond through the fifteen windows: deep
// green water with light playing on it, lily pads, six koi of different kinds (kohaku, ogon,
// showa, tancho, chagoi, shiro utsuri) swimming slow curves, their shadows on the bottom. A press
// is food: the koi gather at that key for a few seconds and a ring spreads on the water (the
// engine's last presses in `aux[0..3]`: x, y, the start on the layer's clock, 1 when on).
fn path(j: f32, t: f32) -> vec2<f32> {
    let size = vec2<f32>(f.size);
    let w1 = 0.05 + 0.03 * hash(vec2<f32>(j, 1.0));
    let w2 = 0.07 + 0.04 * hash(vec2<f32>(j, 2.0));
    let a = vec2<f32>(sin(t * w1 + j * 2.3), sin(t * w2 + j * 4.1 + 1.0));
    let b = vec2<f32>(sin(t * w1 * 2.1 + j), cos(t * w2 * 1.7 + j * 0.7));
    return size * 0.5 + vec2<f32>(size.x * 0.4, size.y * 0.36) * (a * 0.8 + b * 0.2);
}
// where the koi j is at time t: its path, drawn to the food after a press
fn place(j: f32, t: f32) -> vec2<f32> {
    var at = path(j, t);
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = t - b.z;
        if (age < 0.0) { continue; }
        let pull = smoothstep(0.2, 2.2, age) * (1.0 - smoothstep(5.0, 8.0, age));
        let orbit = b.xy + vec2<f32>(cos(t * 0.9 + j * 1.05), sin(t * 0.9 + j * 1.05)) * (16.0 + 7.0 * hash(vec2<f32>(j, 9.0)));
        at = mix(at, orbit, pull);
    }
    return at;
}
fn koi(q: vec2<f32>, len: f32, wag: f32) -> vec2<f32> {
    // q in fish space: the head at -x. Returns the body's distance and the tail's.
    let u = clamp((q.x + len * 0.5) / len, 0.0, 1.0);
    let bend = sin(wag + u * 3.0) * u * u * len * 0.12;
    let y = q.y - bend;
    let half = len * 0.16 * sin(3.14159 * pow(u, 0.75)) + 0.5;
    let body = max(abs(y) - half, abs(q.x) - len * 0.5);
    // the tail fan beyond the body, and two pectoral fins near the head
    let tq = vec2<f32>(q.x - len * 0.5, q.y - bend);
    let tail = max(abs(tq.y) - (1.0 + tq.x * 0.75), max(-tq.x, tq.x - len * 0.24));
    let fq = vec2<f32>(q.x + len * 0.18, abs(q.y) - len * 0.13);
    let fins = length(fq / vec2<f32>(len * 0.1, len * 0.05)) - 1.0;
    return vec2<f32>(body, min(tail, fins * len * 0.05));
}
fn pattern(kind: i32, q: vec2<f32>, len: f32, seed: f32) -> vec3<f32> {
    let white = vec3<f32>(0.96, 0.95, 0.92);
    let red = vec3<f32>(0.92, 0.22, 0.06);
    let black = vec3<f32>(0.06, 0.06, 0.07);
    let n = vnoise(q / len * 5.0 + seed * 7.0);
    switch kind {
        case 0: { return select(white, red, n > 0.52); }
        case 1: { return mix(vec3<f32>(1.0, 0.72, 0.22), vec3<f32>(1.0, 0.92, 0.6), smoothstep(0.3, 0.8, n)); }
        case 2: { return select(select(white, red, n > 0.45), black, vnoise(q / len * 4.0 + seed * 3.0) > 0.6); }
        case 3: { return select(white, red, length(q - vec2<f32>(-len * 0.3, 0.0)) < len * 0.1); }
        case 4: { return mix(vec3<f32>(0.62, 0.42, 0.22), vec3<f32>(0.78, 0.55, 0.3), n); }
        default: { return select(white, black, n > 0.55); }
    }
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time;
    // the water, seen from above: a bottom of rounded pebbles through deep green water (deeper,
    // darker towards the middle), a net of sunlight playing on it, the sky's sheen on the surface
    let n1 = vnoise(p * 0.02 + vec2<f32>(t * 0.03, 0.0));
    let bend = vec2<f32>(vnoise(p * 0.05 + vec2<f32>(t * 0.2, 0.0)), vnoise(p * 0.05 + vec2<f32>(0.0, t * 0.17) + 7.0)) * 3.0;
    let pp = (p + bend) / 11.0;
    let pc = floor(pp);
    var near = 9.0;
    var second = 9.0;
    var stone = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let g = pc + vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(hash(g + 0.3), hash(g + 5.1)) * 0.8 + 0.1;
            let dd = length(pp - g - o);
            if (dd < near) { second = near; near = dd; stone = hash(g + 9.7); } else if (dd < second) { second = dd; }
        }
    }
    // each pebble rounded (lighter in the middle), grey to brown, dark gaps between them
    let edge = second - near;
    var bottom = mix(vec3<f32>(0.24, 0.24, 0.2), vec3<f32>(0.36, 0.3, 0.22), stone) * (0.55 + 0.45 * smoothstep(0.0, 0.5, edge)) * (0.85 + 0.25 * smoothstep(0.6, 0.0, near));
    let deep = smoothstep(0.0, 1.0, 1.0 - length((p - vec2<f32>(f.size) * 0.5) / vec2<f32>(f.size)) * 1.6);
    var water = mix(bottom * vec3<f32>(0.6, 0.8, 0.66), vec3<f32>(0.03, 0.13, 0.12), 0.28 + 0.34 * deep);
    water = mix(water, vec3<f32>(0.05, 0.2, 0.17), n1 * 0.3);
    // the sunlight's net on the bottom: thin bright lines where two moving waves meet
    let w1 = vnoise(p * 0.07 + vec2<f32>(t * 0.35, t * 0.2));
    let w2 = vnoise(p * 0.07 + vec2<f32>(-t * 0.25, t * 0.3) + 13.0);
    // wider and brighter where the waves gather, fading out in between, like real caustics
    let gather = smoothstep(0.3, 0.8, vnoise(p * 0.025 + vec2<f32>(t * 0.08, -t * 0.05)));
    let caustic = smoothstep(0.78 - 0.1 * gather, 1.0, 1.0 - abs(w1 - w2) * 2.2) * (0.35 + 0.65 * gather);
    water += vec3<f32>(0.26, 0.36, 0.24) * caustic * (1.0 - 0.5 * deep);
    // the sky's sheen across the surface, from the upper left
    water += vec3<f32>(0.08, 0.12, 0.13) * smoothstep(0.55, 0.0, (p.x + p.y * 1.4) / (f32(f.size.x) + f32(f.size.y) * 1.4)) * (0.6 + 0.4 * vnoise(p * 0.03 + t * 0.05));
    // the koi's shadows on the bottom first, then the koi
    var col = water;
    for (var j = 0; j < 6; j++) {
        let fj = f32(j);
        let at = place(fj, t);
        let ahead = place(fj, t + 0.15);
        let heading = normalize(ahead - at + vec2<f32>(0.0001, 0.0));
        let len = 34.0 + 8.0 * hash(vec2<f32>(fj, 5.0));
        let d0 = p - at - vec2<f32>(5.0, 7.0);
        if (dot(d0, d0) < len * len) {
            let q = vec2<f32>(-dot(d0, heading), d0.x * heading.y - d0.y * heading.x);
            let s = koi(q, len, t * 6.0 + fj);
            col *= 1.0 - 0.35 * smoothstep(3.0, -2.0, min(s.x, s.y));
        }
    }
    for (var j = 0; j < 6; j++) {
        let fj = f32(j);
        let at = place(fj, t);
        let ahead = place(fj, t + 0.15);
        let heading = normalize(ahead - at + vec2<f32>(0.0001, 0.0));
        let len = 34.0 + 8.0 * hash(vec2<f32>(fj, 5.0));
        let d0 = p - at;
        if (dot(d0, d0) > len * len) { continue; }
        let q = vec2<f32>(-dot(d0, heading), d0.x * heading.y - d0.y * heading.x);
        let speed = length(ahead - at) / 0.15;
        let s = koi(q, len, t * (4.0 + speed * 0.08) + fj);
        var fish = pattern(j, q, len, fj);
        // rounded back: lighter along the spine, the fins and tail see-through
        fish *= 0.78 + 0.3 * smoothstep(len * 0.16, 0.0, abs(q.y));
        col = mix(col, mix(water, fish, 0.55), smoothstep(0.7, -0.7, s.y));
        col = mix(col, fish, smoothstep(0.7, -0.7, s.x));
    }
    // lily pads: still, round with a notch, darker veins, resting on the water
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let at = vec2<f32>(f32(f.size.x) * (0.12 + 0.27 * fi), f32(f.size.y) * (0.25 + 0.5 * hash(vec2<f32>(fi, 4.0)))) + vec2<f32>(sin(t * 0.05 + fi), cos(t * 0.04 + fi)) * 6.0;
        let r = 17.0 + 6.0 * hash(vec2<f32>(fi, 7.0));
        let d0 = p - at;
        let d = length(d0) - r;
        if (d > 3.0) { continue; }
        let a = atan2(d0.y, d0.x) - fi * 1.7;
        // the notch: a narrow wedge to the middle, its sides smooth (a hard cut stair-stepped)
        let ca = vec2<f32>(cos(fi * 1.7), sin(fi * 1.7));
        let along = dot(d0, ca);
        let across = abs(d0.x * ca.y - d0.y * ca.x);
        let notch = across - along * 0.16 - 0.4;
        let dp = max(d, -select(1e3, notch, along > 0.0));
        // a soft shadow on the water, then the pad: green, darker veins from the middle, a lighter
        // rim turned up a little, a wet sheen on the sunny side
        col *= 1.0 - 0.35 * smoothstep(3.0, -1.0, length(d0 - vec2<f32>(-3.0, -4.0)) - r);
        var pad = mix(vec3<f32>(0.12, 0.3, 0.1), vec3<f32>(0.26, 0.44, 0.14), vnoise(d0 * 0.15 + fi));
        pad *= 1.0 - 0.18 * smoothstep(0.25, 0.0, abs(fract(a * 2.5) - 0.5)) * smoothstep(1.5, 6.0, length(d0));
        pad = mix(pad, vec3<f32>(0.34, 0.52, 0.2), smoothstep(-3.0, -0.5, d) * 0.6);
        pad += vec3<f32>(0.1, 0.12, 0.08) * smoothstep(r, 0.0, length(d0 + vec2<f32>(r * 0.35, r * 0.35))) * 0.5;
        col = mix(col, pad, smoothstep(0.7, -0.7, dp));
        // a lotus flower on two of the pads: pink petals round a yellow heart
        if (i % 2 == 0) {
            let fc = d0 - vec2<f32>(r * 0.25, -r * 0.2);
            let fa = atan2(fc.y, fc.x);
            let petals = length(fc) - 6.5 * (0.7 + 0.3 * abs(cos(fa * 4.0 + fi)));
            var lotus = mix(vec3<f32>(0.95, 0.62, 0.74), vec3<f32>(0.8, 0.28, 0.48), smoothstep(6.0, 1.0, length(fc)));
            lotus = mix(vec3<f32>(0.95, 0.8, 0.3), lotus, smoothstep(1.2, 2.2, length(fc)));
            col *= 1.0 - 0.25 * smoothstep(2.0, -1.0, length(fc - vec2<f32>(-2.0, -2.5)) - 7.0);
            col = mix(col, lotus, smoothstep(0.6, -0.6, petals));
        }
    }
    // a ring on the water where the food fell
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = t - b.z;
        if (age < 0.0 || age > 2.5) { continue; }
        let ring = abs(length(p - b.xy) - age * 38.0);
        col += vec3<f32>(0.5, 0.65, 0.6) * smoothstep(2.0, 0.0, ring) * (1.0 - age / 2.5) * 0.6;
    }
    c = mix(c, col, f.strength / 0.6 * 0.95 * (1.0 - keep_bg(c)));
    put(id.xy, clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
}
