// Poland B, a mushroom hunt: inside an old mixed forest in the morning, one view through the fifteen
// windows. Pine and beech trunks rise through all three rows in three depths (far ones pale in the
// haze, near ones dark, barked, mossy at the foot, their sunny side lit), the canopy a dark roof
// with gaps of light at the top, slanting shafts of sun from the upper left with motes in them,
// ground fog drifting between the trunks, and a floor of moss, ferns, needles and leaf litter in the
// bottom row (the mushrooms layer grows there). Held dark by a soft knee, so icons stay readable.
const LIGHT = vec2<f32>(-0.42, 1.0);
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
// a trunk's x at height y: a gentle lean and a little wander
fn trunk_x(x0: f32, y: f32, seed: f32) -> f32 {
    return x0 + (y - 136.0) * (hash(vec2<f32>(seed, 2.0)) - 0.5) * 0.06 + 2.0 * (vnoise(vec2<f32>(y * 0.012, seed)) - 0.5);
}
struct Trunk {
    x: f32,
    w: f32,
    pine: bool,
}
// three depths: 0 far (many, thin), 1 middle, 2 near (few, wide)
fn trunk_at(depth: i32, i: i32) -> Trunk {
    let fi = f32(i) + f32(depth) * 31.0;
    var count = 11.0;
    var w = 3.0;
    if (depth == 1) { count = 6.0; w = 7.0; }
    if (depth == 2) { count = 4.0; w = 14.0; }
    let span = 520.0 / count;
    let x = (f32(i) + 0.2 + 0.6 * hash(vec2<f32>(fi, 3.0))) * span - 20.0;
    return Trunk(x, w * (0.8 + 0.4 * hash(vec2<f32>(fi, 5.0))), hash(vec2<f32>(fi, 7.0)) < 0.55);
}
fn trunk_count(depth: i32) -> i32 {
    if (depth == 0) { return 11; }
    if (depth == 1) { return 6; }
    return 4;
}
// a fern frond from `base`, leaning by `lean`, of length `len`: a curved stalk with leaflets that
// shorten towards the tip; the distance (px) and how far along (0..1)
fn frond(p: vec2<f32>, base: vec2<f32>, len: f32, lean: f32) -> vec2<f32> {
    let q = p - base;
    // the stalk rises and arches over: x = lean * len * t^2, y = -len * (t - 0.35 t^2)
    var best = 1e3;
    var along = 0.0;
    for (var j = 0; j < 9; j++) {
        let t = f32(j) / 8.0;
        let c = vec2<f32>(lean * len * t * t, -len * (t - 0.35 * t * t));
        let tn = normalize(vec2<f32>(2.0 * lean * len * t, -len * (1.0 - 0.7 * t)) + vec2<f32>(0.0001));
        let side = vec2<f32>(-tn.y, tn.x);
        let r = q - c;
        let s = dot(r, side);
        let a = dot(r, tn);
        if (abs(a) < len / 14.0) {
            // the leaflets: lobes out to each side, shorter towards the tip
            let reach = len * 0.24 * (1.0 - t) * (0.55 + 0.45 * sin(t * 3.14159 + 0.4));
            let lobe = 0.6 + 0.4 * cos(a / (len / 14.0) * 3.14159);
            let d = abs(s) - reach * lobe;
            if (d < best) { best = d; along = t; }
        }
    }
    return vec2<f32>(best, along);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let tm = f.time * f.speed;
    // --- the far forest in the morning haze: lighter and cooler high up, deep green low down
    let v = p.y / 272.0;
    var col = mix(vec3<f32>(0.62, 0.58, 0.4), vec3<f32>(0.1, 0.13, 0.06), smoothstep(0.1, 0.85, v));
    col += vec3<f32>(0.3, 0.24, 0.1) * exp(-length(p - vec2<f32>(60.0, -20.0)) / 170.0);
    // far autumn leaves: dabs of beech gold and maple rust in the haze
    let dab = fbm(vec2<f32>(p.x * 0.05, p.y * 0.07) + 5.0);
    col = mix(col, mix(vec3<f32>(0.62, 0.42, 0.14), vec3<f32>(0.58, 0.24, 0.1), vnoise(p * 0.03)), smoothstep(0.58, 0.72, dab) * smoothstep(200.0, 40.0, p.y) * 0.55);
    // nearer foliage: clumps of beech gold and maple rust hanging between the trunks, lit from the
    // upper left, their edges leafy (never pixel noise)
    let leafy = fbm(vec2<f32>(p.x * 0.028, p.y * 0.04) + 21.0);
    let clump = smoothstep(0.56, 0.62, leafy + 0.06 * (vnoise(p * 0.18) - 0.5)) * smoothstep(175.0, 70.0, p.y) * smoothstep(0.0, 30.0, p.y);
    let hue = vnoise(vec2<f32>(p.x * 0.012, 3.0));
    var foliage = mix(vec3<f32>(0.78, 0.5, 0.12), vec3<f32>(0.66, 0.22, 0.07), smoothstep(0.35, 0.75, hue));
    foliage *= 0.55 + 0.6 * smoothstep(0.5, 0.75, fbm(vec2<f32>(p.x * 0.06, p.y * 0.08) + 4.0)) * smoothstep(250.0, 0.0, p.x + p.y * 0.5);
    col = mix(col, foliage, clump * 0.85);
    col *= 0.85 + 0.3 * fbm(vec2<f32>(p.x * 0.02, p.y * 0.006));
    // the canopy: a dark roof with gaps of sky, densest at the top
    let leaves = fbm(vec2<f32>(p.x * 0.035 + tm * 0.01, p.y * 0.05));
    let roof = smoothstep(0.42, 0.62, leaves) * smoothstep(110.0, 10.0, p.y);
    col = mix(col, vec3<f32>(0.06, 0.1, 0.04), roof * 0.85);
    let gap = smoothstep(0.32, 0.22, leaves) * smoothstep(70.0, 0.0, p.y);
    col = mix(col, vec3<f32>(0.85, 0.9, 0.7), gap * 0.7);
    // --- the trunks, far to near; each its bark, its sunny side lit, moss at the foot
    for (var depth = 0; depth < 3; depth++) {
        let haze = f32(2 - depth) / 2.0;
        for (var i = 0; i < trunk_count(depth); i++) {
            let t = trunk_at(depth, i);
            let x = trunk_x(t.x, p.y, f32(i) + f32(depth) * 9.0);
            // wider at the foot, a root flare on the near ones
            let foot = smoothstep(150.0, 272.0, p.y);
            let w = t.w * (1.0 + 0.25 * foot + select(0.0, 0.5, depth == 2) * smoothstep(225.0, 272.0, p.y));
            let d = abs(p.x - x) - w;
            if (d > 1.0) { continue; }
            let u = (p.x - x) / w;
            // round: the sun from the upper left lights the left side
            let lit = clamp(0.5 - u * 0.6, 0.0, 1.0);
            var bark: vec3<f32>;
            if (t.pine) {
                // pine: grey-brown deep plates below, orange thin bark high up
                let plates = fbm(vec2<f32>(u * 3.0, p.y * 0.09 / max(t.w / 8.0, 0.5)) + f32(i));
                bark = mix(vec3<f32>(0.2, 0.15, 0.11), vec3<f32>(0.36, 0.26, 0.18), plates);
                bark = mix(bark, vec3<f32>(0.62, 0.34, 0.16), smoothstep(150.0, 30.0, p.y) * 0.75);
                bark *= 1.0 - 0.45 * smoothstep(0.58, 0.68, plates);
            } else {
                // beech: smooth grey, faint horizontal lines
                bark = vec3<f32>(0.42, 0.43, 0.4) * (0.92 + 0.12 * vnoise(vec2<f32>(u * 2.0, p.y * 0.04) + f32(i)));
                bark *= 1.0 - 0.12 * smoothstep(0.75, 0.9, vnoise(vec2<f32>(p.x * 0.1, p.y * 0.6)));
            }
            bark *= select(0.35 + 0.85 * lit, 0.22 + 0.7 * lit, depth == 2);
            // moss climbing the foot on the shaded side
            let moss = smoothstep(200.0, 262.0, p.y + 30.0 * fbm(vec2<f32>(u * 2.0, p.y * 0.05))) * smoothstep(-0.6, 0.6, u);
            bark = mix(bark, vec3<f32>(0.16, 0.26, 0.06) * (0.6 + 0.6 * lit), moss * 0.85);
            // the haze of distance
            bark = mix(bark, col * 0.92 + vec3<f32>(0.02, 0.03, 0.02), haze * 0.55);
            col = mix(col, bark, smoothstep(0.9, -0.9, d));
        }
    }
    // --- the floor in the bottom row: moss and needles, ferns, a little leaf litter
    let floor_y = 214.0 + 5.0 * sin(p.x * 0.017) + 3.0 * sin(p.x * 0.05 + 1.0);
    if (p.y > floor_y - 2.0) {
        let g = fbm(vec2<f32>(p.x * 0.06, p.y * 0.12));
        var ground = mix(vec3<f32>(0.08, 0.13, 0.04), vec3<f32>(0.24, 0.32, 0.08), g);
        // needles and litter: rust flecks
        // fallen leaves: a small turned oval in some cells (a whole cell made square blocks)
        let cs = vec2<f32>(8.0, 4.5);
        let cell = floor(p / cs);
        if (hash(cell) > 0.62) {
            let lc = mix(vec3<f32>(0.5, 0.22, 0.07), vec3<f32>(0.62, 0.44, 0.12), hash(cell + 5.0));
            let lp = (cell + vec2<f32>(0.25 + 0.5 * hash(cell + 3.1), 0.3 + 0.4 * hash(cell + 4.2))) * cs;
            let ang = hash(cell + 6.3) * 3.14159;
            let d = p - lp;
            let r = vec2<f32>(d.x * cos(ang) + d.y * sin(ang), -d.x * sin(ang) + d.y * cos(ang)) / vec2<f32>(2.8, 1.3);
            ground = mix(ground, lc * (0.8 + 0.3 * smoothstep(0.0, -1.0, r.y)), smoothstep(1.0, 0.7, length(r)) * 0.85);
        }
        // the floor gets darker towards you
        ground *= 0.75 + 0.35 * smoothstep(272.0, floor_y, p.y);
        col = mix(col, ground, smoothstep(floor_y - 1.0, floor_y + 1.0, p.y));
    }
    // ferns rising from the floor, in front of the trunks' feet
    for (var i = 0; i < 7; i++) {
        let fi = f32(i);
        let bx = (fi + 0.3 + 0.4 * hash(vec2<f32>(fi, 61.0))) * 72.0 - 10.0;
        let base = vec2<f32>(bx, 262.0 + 6.0 * hash(vec2<f32>(fi, 62.0)));
        for (var j = 0; j < 3; j++) {
            let fj = f32(j);
            let lean = (fj - 1.0) * 0.9 + (hash(vec2<f32>(fi, fj + 63.0)) - 0.5) * 0.4 + 0.04 * sin(tm * 0.9 + fi + fj);
            let len = 40.0 + 20.0 * hash(vec2<f32>(fj, fi + 64.0));
            let fr = frond(p, base, len, lean);
            if (fr.x < 0.9) {
                var fc = mix(vec3<f32>(0.1, 0.2, 0.04), vec3<f32>(0.34, 0.46, 0.1), fr.y);
                // a few already turning gold
                if (hash(vec2<f32>(fi, 65.0 + fj)) > 0.45) { fc = mix(fc, mix(vec3<f32>(0.62, 0.3, 0.08), vec3<f32>(0.72, 0.5, 0.12), fr.y), 0.75); }
                col = mix(col, fc, smoothstep(0.9, -0.9, fr.x));
            }
        }
    }
    // --- ground fog between the trunks, drifting
    let fogn = fbm(vec2<f32>(p.x * 0.012 - tm * 0.03, p.y * 0.03));
    let fog = smoothstep(120.0, 225.0, p.y) * smoothstep(272.0, 230.0, p.y) * (0.35 + 0.65 * fogn);
    col = mix(col, vec3<f32>(0.62, 0.66, 0.56), fog * 0.4);
    // --- shafts of sun from the upper left through the canopy, slowly breathing, motes in them
    let along = dot(p, normalize(vec2<f32>(LIGHT.y, -LIGHT.x)));
    let shaft = smoothstep(0.62, 0.86, vnoise(vec2<f32>(along * 0.035, tm * 0.05))) * (0.5 + 0.5 * smoothstep(272.0, 0.0, p.y));
    col += vec3<f32>(1.0, 0.76, 0.36) * shaft * 0.42 * (0.6 + 0.4 * fog + 0.3 * smoothstep(0.0, 1.0, 1.0 - v));
    let mc = floor((p + vec2<f32>(tm * 2.0, -tm * 1.0)) / 11.0);
    let mh = hash(mc);
    if (mh > 0.86) {
        let mp = (mc + vec2<f32>(hash(mc + 1.3), hash(mc + 2.7))) * 11.0 - vec2<f32>(tm * 2.0, -tm * 1.0);
        let tw = 0.5 + 0.5 * sin(tm * (1.5 + mh * 3.0) + mh * 40.0);
        col += vec3<f32>(1.0, 0.9, 0.6) * smoothstep(1.1, 0.0, length(p - mp)) * tw * shaft * 0.9;
    }
    col *= 1.0 - 0.35 * smoothstep(0.35, 0.95, length((p - vec2<f32>(220.0, 120.0)) / vec2<f32>(300.0, 190.0)));
    // dark by design: a soft knee near 40 %
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.36) { col *= (0.36 + (lum - 0.36) * 0.3) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
