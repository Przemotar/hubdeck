// The painter's studio, for creators: luminous watercolour inks on a sheet of black watercolour
// paper behind the icons, one sheet through the fifteen windows. The paper's grain catches a soft
// light; washes of ultramarine, rose, viridian, gold ochre and violet lie over it, each smooth
// inside with the pigment pooled towards its edge and a thin, darker, sharper line where it dried
// (the "coffee ring" of real watercolour), a soft backrun bloom in some, glazes mixing where they
// overlap, slowly breathing and drifting. A press: a drop of paint lands on the key and blooms
// out over the next keys, its wet edge shining, then dries into the sheet (`aux[0..3]`: x, y,
// start, 1 when on).
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        v += a * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.1, 9.3);
        a *= 0.5;
    }
    return v;
}
fn pigment(i: i32) -> vec3<f32> {
    switch i {
        case 0: { return vec3<f32>(0.2, 0.36, 0.95); }
        case 1: { return vec3<f32>(0.95, 0.24, 0.5); }
        case 2: { return vec3<f32>(0.1, 0.72, 0.58); }
        case 3: { return vec3<f32>(0.98, 0.68, 0.2); }
        default: { return vec3<f32>(0.62, 0.36, 0.95); }
    }
}
// a wash at c of radius r with a wandering edge; .x the pigment (pooled towards the edge), .y the
// dried edge line, .z inside (0..1)
fn wash(p: vec2<f32>, c: vec2<f32>, r: f32, seed: f32) -> vec3<f32> {
    let q = p - c;
    let a = atan2(q.y, q.x);
    // the edge wanders in big lobes and small bays, never in pixel noise
    let lobe = 0.16 * (vnoise(vec2<f32>(a * 1.6 + seed, seed)) - 0.5) + 0.07 * (vnoise(vec2<f32>(a * 5.0 + seed * 2.0, 1.0)) - 0.5);
    let warp = 0.1 * (fbm(q / r * 1.3 + seed) - 0.5);
    let d = length(q) / r * (1.0 - lobe) + warp;
    let px_from_edge = (1.0 - d) * r;
    let inside = smoothstep(-0.8, 0.8, px_from_edge);
    let pool = 0.42 + 0.3 * smoothstep(0.55, 0.97, d);
    let line = exp(-abs(px_from_edge - 0.8) / 1.1) * inside;
    return vec3<f32>(pool * inside, line, inside);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * f.speed;
    // the black paper: a smooth grain catching a soft light from the upper left
    let grain = fbm(p * 0.22) - 0.5;
    var col = vec3<f32>(0.04, 0.038, 0.045) * (1.0 + 0.35 * grain);
    col += vec3<f32>(0.025, 0.022, 0.02) * smoothstep(480.0, 0.0, p.x + p.y * 1.3);
    // the washes, glazed one over another: each tints what is under it, the edges darker
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        // spread over the sheet: one in each of six places (three across, two down), jittered
        let cell = vec2<f32>(f32(i % 3), f32(i / 3));
        let ctr = (cell + vec2<f32>(0.25 + 0.5 * hash(vec2<f32>(fi, 1.0)), 0.25 + 0.5 * hash(vec2<f32>(fi, 2.0)))) * vec2<f32>(160.0, 136.0) + vec2<f32>(sin(t * 0.05 + fi), cos(t * 0.04 + fi * 1.3)) * 14.0;
        let r = 44.0 + 34.0 * hash(vec2<f32>(fi, 3.0)) + 5.0 * sin(t * 0.08 + fi * 2.0);
        // a brushstroke, not a puddle: the wash drawn out along a turned, gently bent line
        let ang = hash(vec2<f32>(fi, 7.0)) * 3.14159;
        let el = 1.7 + 1.1 * hash(vec2<f32>(fi, 8.0));
        let bend = (hash(vec2<f32>(fi, 9.0)) - 0.5) * 0.012;
        var q = p - ctr;
        q = vec2<f32>(q.x * cos(ang) + q.y * sin(ang), -q.x * sin(ang) + q.y * cos(ang));
        q.y += bend * q.x * q.x;
        q.x /= el;
        let w = wash(ctr + q, ctr, r, fi * 3.7);
        if (w.z <= 0.0) { continue; }
        let pg = pigment(i32(fi) % 5);
        // granulation: pigment settles a little in the paper's grain, never speckled
        let settle = 1.0 + 0.18 * grain;
        // a soft backrun bloom inside some washes: a paler pool with a feathered rim
        var bloom = 0.0;
        if (hash(vec2<f32>(fi, 9.0)) > 0.45) {
            let bw = wash(p, ctr + vec2<f32>(r * 0.25, -r * 0.2), r * 0.42, fi * 5.1);
            bloom = bw.z * 0.45 - bw.y * 0.25;
        }
        // the pigment never lies even: a slow cloudiness inside the wash
        let cloud = 0.75 + 0.5 * fbm(p * 0.018 + vec2<f32>(fi * 4.0, t * 0.01));
        let load = clamp(w.x * settle * cloud * (1.0 - bloom) + w.y * 0.45, 0.0, 1.0);
        // glazing: the new colour over the old, the old showing through
        col = col * (1.0 - load * 0.45) + pg * load * 0.62;
    }
    // a few splatters: small round drops with their own little edge
    let sc = floor(p / 30.0);
    let sh = hash(sc + 2.2);
    if (sh > 0.8) {
        let sp = (sc + vec2<f32>(0.2 + 0.6 * hash(sc + 1.1), 0.2 + 0.6 * hash(sc + 3.3))) * 30.0;
        let sr = 1.3 + 2.8 * hash(sc + 5.5);
        let w = wash(p, sp, sr, sh * 30.0);
        col = col * (1.0 - w.z * 0.3) + pigment(i32(sh * 50.0) % 5) * w.z * 0.42;
    }
    // a press: a drop lands and blooms over the next keys, its wet edge shining, then dries
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0) { continue; }
        let grow = 1.0 - exp(-age * 1.7);
        let r = 6.0 + 88.0 * grow;
        let w = wash(p, b.xy, r, b.z * 1.3);
        let pg = pigment(i32(b.z * 7.0) % 5);
        let wet = 1.0 - smoothstep(0.8, 2.6, age);
        let dry = 1.0 - smoothstep(2.4, 3.8, age);
        let load = (w.x * (1.0 + 0.18 * grain) + w.y * (0.45 + 0.5 * wet)) * dry;
        col = col * (1.0 - load * 0.45) + pg * load * 0.7;
        // the wet sheen near the middle while it is still wet
        col += vec3<f32>(0.3, 0.3, 0.32) * wet * dry * w.z * exp(-length(p - b.xy - vec2<f32>(-r * 0.2, -r * 0.25)) / max(r * 0.3, 2.0)) * 0.3;
    }
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.4) { col *= (0.4 + (lum - 0.4) * 0.35) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
