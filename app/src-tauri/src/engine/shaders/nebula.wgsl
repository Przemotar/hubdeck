// Space, for sim players: deep space through the fifteen windows. A nebula of glowing gas (teal,
// violet, rose) with dark dust lanes across it, three layers of stars drifting at their own speeds
// (parallax: near ones bigger, quicker, a few with diffraction spikes), and a ringed planet behind
// the keys on the right, lit from the left, its night side dark, its rings casting a shadow. A
// press: a jump to hyperspace from the key, the stars stretching into streaks that run out from it
// and snap back (`aux[0..3]`: x, y, start, 1 when on).
const PLANET = vec2<f32>(392.0, 168.0);
const PLANET_R = 58.0;
fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        v += a * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.1, 9.3);
        a *= 0.5;
    }
    return v;
}
// a layer of stars in cells of `cell` px drifting by `drift`; their light at p
fn stars(p: vec2<f32>, cell: f32, drift: vec2<f32>, size: f32, seed: f32, warp: vec2<f32>) -> vec3<f32> {
    let q = p + drift;
    let g = floor(q / cell);
    var light = vec3<f32>(0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let gc = g + vec2<f32>(f32(i), f32(j));
            let h = hash(gc + seed);
            if (h < 0.55) { continue; }
            let sp = (gc + vec2<f32>(hash(gc + seed + 1.3), hash(gc + seed + 2.7))) * cell;
            var d = q - sp;
            // in hyperspace a star is a streak along the way out from the jump
            let wl = length(warp);
            if (wl > 0.01) {
                let dir = normalize(sp - drift - warp * 0.0 - (p - d) + d + vec2<f32>(0.0001));
                let along = dot(d, normalize(warp));
                let across = d - normalize(warp) * clamp(along, -wl, 0.0);
                d = across;
            }
            let r = size * (0.5 + h);
            let tw = 0.75 + 0.25 * sin(f.time * (2.0 + h * 4.0) + h * 30.0);
            let tint = mix(vec3<f32>(0.7, 0.8, 1.0), vec3<f32>(1.0, 0.85, 0.7), hash(gc + seed + 4.4));
            light += tint * exp(-dot(d, d) / (r * r)) * tw * (0.6 + 0.8 * (h - 0.55) / 0.45);
            // the brightest near ones get four spikes
            if (h > 0.97 && size > 1.0) {
                let a = abs(d);
                light += tint * 0.5 * (exp(-a.y * 1.5) * exp(-a.x / 7.0) + exp(-a.x * 1.5) * exp(-a.y / 7.0));
            }
        }
    }
    return light;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * f.speed;
    // the nebula: glowing gas in three colours, dark dust lanes over it
    let gas1 = fbm(p * 0.008 + vec2<f32>(t * 0.004, 0.0));
    let gas2 = fbm(p * 0.012 + vec2<f32>(5.0, t * 0.003));
    let dust = fbm(p * 0.02 + vec2<f32>(9.0 - t * 0.003, 2.0));
    var col = vec3<f32>(0.006, 0.008, 0.02);
    col += vec3<f32>(0.05, 0.35, 0.42) * smoothstep(0.45, 0.85, gas1) * 0.7;
    col += vec3<f32>(0.42, 0.12, 0.55) * smoothstep(0.42, 0.82, gas2) * 0.6;
    col += vec3<f32>(0.6, 0.18, 0.3) * smoothstep(0.6, 0.9, gas1 * gas2 * 1.6) * 0.5;
    col *= 1.0 - 0.8 * smoothstep(0.5, 0.72, dust);
    // the jump: where it runs from and how far the stars stretch
    var warp = vec2<f32>(0.0);
    var flash = 0.0;
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 1.6) { continue; }
        let k = sin(clamp(age / 1.3, 0.0, 1.0) * 3.14159);
        let out = p - b.xy;
        warp += normalize(out + vec2<f32>(0.0001)) * k * (8.0 + length(out) * 0.35);
        flash = max(flash, exp(-age * 6.0) * exp(-length(out) / 60.0));
    }
    // three layers of stars: far (tiny, slow), middle, near (bigger, quicker)
    col += stars(p, 9.0, vec2<f32>(t * 1.0, 0.0), 0.55, 1.0, warp * 0.4);
    col += stars(p, 17.0, vec2<f32>(t * 2.6, t * 0.4), 0.8, 7.0, warp * 0.7);
    col += stars(p, 34.0, vec2<f32>(t * 6.0, t * 1.0), 1.2, 13.0, warp);
    col += vec3<f32>(0.7, 0.85, 1.0) * flash * 0.6;
    // the planet: banded, lit from the upper left, its night side dark, an atmosphere's rim
    let q = p - PLANET;
    let dp = length(q) - PLANET_R;
    // the rings, tilted: an ellipse band, behind the planet at the top, in front at the bottom
    let rq = vec2<f32>(q.x * 0.94 + q.y * 0.34, (-q.x * 0.34 + q.y * 0.94) * 3.4);
    let rr = length(rq);
    let ring = smoothstep(PLANET_R * 1.25, PLANET_R * 1.3, rr) * smoothstep(PLANET_R * 2.05, PLANET_R * 1.95, rr);
    let ringc = vec3<f32>(0.62, 0.55, 0.44) * (0.6 + 0.4 * sin(rr * 0.6)) * (0.7 + 0.3 * vnoise(vec2<f32>(rr * 0.9, 1.0)));
    let front = rq.y > 0.0;
    if (!front) { col = mix(col, ringc * 0.8, ring * 0.85); }
    if (dp < 1.0) {
        let n = vec3<f32>(q / PLANET_R, sqrt(max(1.0 - dot(q, q) / (PLANET_R * PLANET_R), 0.0)));
        let light = normalize(vec3<f32>(-0.7, -0.45, 0.55));
        let lit = clamp(dot(n, light), 0.0, 1.0);
        let lat = n.y + 0.08 * fbm(vec2<f32>(n.x * 3.0 + t * 0.01, n.y * 12.0));
        var surf = mix(vec3<f32>(0.72, 0.52, 0.34), vec3<f32>(0.42, 0.26, 0.2), 0.5 + 0.5 * sin(lat * 18.0));
        surf = mix(surf, vec3<f32>(0.85, 0.75, 0.6), smoothstep(0.6, 0.9, vnoise(vec2<f32>(lat * 30.0, 2.0))) * 0.3);
        // the rings' shadow across it
        let sh = smoothstep(0.03, 0.0, abs(n.y * 0.94 - n.x * 0.34 + 0.18)) * 0.6;
        var pc = surf * (0.04 + 1.05 * pow(lit, 0.9)) * (1.0 - sh);
        pc += vec3<f32>(0.35, 0.55, 1.0) * pow(1.0 - n.z, 3.0) * (0.25 + 0.75 * lit) * 0.6;
        col = mix(col, pc, smoothstep(0.8, -0.8, dp));
    }
    if (front) { col = mix(col, ringc, ring * 0.9 * select(1.0, 0.0, dp < 0.0 && false)); }
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.45) { col *= (0.45 + (lum - 0.45) * 0.4) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
