const STONES = false;
// The koi pond as one picture in the 480x480 world square (`world()`; owner, 2026-10-29: "liście
// lotosu i kwiaty mogą być też na tych poza, ale ryby nie mogą wypływać poza aktualny kadr"): the
// water, its bottom, the lily pads and lotus flowers (and the stones round the pond) lie in the
// square, so turning the deck shows another crop of the same pond; the koi swim in what the deck
// shows now (`to_view`), never out of it. Seen from above: deep green water over a pebble bottom
// with the sun's net of light on it and the sky's sheen on the surface; lily pads with veins, rims
// turned up and drops of water; pink lotus flowers; six koi of their kinds (kohaku, showa, sanke,
// ogon, asagi, chagoi) with fins and veil tails, scales and a sheen down the back, their shadows on
// the bottom. A press is food: pellets fall, rings spread, the koi come to that key for a while; the
// lily pads rock when a koi passes under them or a ring reaches them. No petals (owner, 2026-10-29:
// "z tych koi chyba usunąłbym te płatki").
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
fn rot(v: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(v.x * c + v.y * s, -v.x * s + v.y * c);
}
// --- the koi swim in the view as it stands, kept a little inside it
fn path(j: f32, t: f32) -> vec2<f32> {
    let size = view_size();
    let w1 = 0.05 + 0.03 * hash(vec2<f32>(j, 1.0));
    let w2 = 0.07 + 0.04 * hash(vec2<f32>(j, 2.0));
    let a = vec2<f32>(sin(t * w1 + j * 2.3), sin(t * w2 + j * 4.1 + 1.0));
    let b = vec2<f32>(sin(t * w1 * 2.1 + j), cos(t * w2 * 1.7 + j * 0.7));
    return size * 0.5 + (size * 0.5 - vec2<f32>(30.0)) * (a * 0.8 + b * 0.2);
}
fn place(j: f32, t: f32) -> vec2<f32> {
    var at = path(j, t);
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = t - b.z;
        if (age < 0.0) { continue; }
        let pull = smoothstep(0.3, 2.4, age) * (1.0 - smoothstep(5.0, 8.0, age));
        let orbit = to_view(b.xy) + vec2<f32>(cos(t * 0.9 + j * 1.05), sin(t * 0.9 + j * 1.05)) * (15.0 + 7.0 * hash(vec2<f32>(j, 9.0)));
        at = mix(at, orbit, pull);
    }
    let lo = vec2<f32>(14.0);
    return clamp(at, lo, view_size() - lo);
}
// a koi in its own space (head at -x, length len, its tail beating by wag): .x the body's distance,
// .y the fins' and tail's, .z across the body (-1..1), .w along it (0 head .. 1 tail root, more in
// the tail)
fn koi(q: vec2<f32>, len: f32, wag: f32) -> vec4<f32> {
    let u = (q.x + len * 0.5) / len;
    let uc = clamp(u, 0.0, 1.3);
    let bend = sin(wag + uc * 3.2) * uc * uc * len * 0.1;
    let y = q.y - bend;
    // the body: a round head, the widest a third back, narrowing to the tail's root
    let head = sqrt(clamp(u / 0.14, 0.0, 1.0));
    let half = len * 0.165 * head * (1.0 - 0.8 * smoothstep(0.32, 0.92, u)) + 0.4;
    let body = max(abs(y) - half, max(-u, u - 0.93) * len);
    // the veil tail: a wide, soft fan past the root, forked at its end
    let tu = clamp((u - 0.86) / 0.4, 0.0, 1.0);
    // two soft lobes, rounded at their ends, a shallow fork between them
    let tw = len * (0.035 + 0.19 * pow(sin(3.14159 * min(tu * 0.62, 1.0)), 0.8));
    let fork = len * 0.05 * smoothstep(0.75, 1.0, tu) * smoothstep(len * 0.06, 0.0, abs(y));
    let tail = max(abs(y) - tw, max(0.86 - u, u - 1.26 + fork / len) * len);
    // the pectoral fins: soft ovals a quarter back, swept out and back, beating
    let fy = abs(y) - len * 0.13;
    let fq = rot(vec2<f32>(q.x + len * 0.22, fy - len * 0.06), 0.7 + 0.25 * sin(wag * 1.3));
    let fins = (length(fq / vec2<f32>(len * 0.14, len * 0.065)) - 1.0) * len * 0.065;
    // the dorsal fin: a thin ridge down the back
    return vec4<f32>(body, min(tail, fins), clamp(y / max(half, 0.5), -1.0, 1.0), u);
}
// a koi's skin by its kind at a point of its body (u along, v across)
fn skin(kind: i32, u: f32, v: f32, len: f32, seed: f32) -> vec3<f32> {
    let white = vec3<f32>(0.96, 0.95, 0.91);
    let red = vec3<f32>(0.9, 0.2, 0.06);
    let black = vec3<f32>(0.05, 0.05, 0.06);
    let pq = vec2<f32>(u * 5.5, v * 1.4) + seed * 7.0;
    let n = fbm(pq);
    switch kind {
        // kohaku: clean white with a few large red patches, the head's own red
        case 0: { return select(white, red, n > 0.5 || (u < 0.16 && v * v + (u - 0.09) * (u - 0.09) * 40.0 < 0.35)); }
        // showa: black ground with red and white bands
        case 1: { return select(select(white, red, n > 0.45), black, fbm(pq * 1.3 + 4.0) > 0.53 || abs(v) > 0.8); }
        // sanke: white and red with small black spots on the back
        case 2: {
            var c = select(white, red, n > 0.52);
            let spot = vnoise(vec2<f32>(u * 22.0, v * 5.0) + seed) > 0.78 && u > 0.2;
            return select(c, black, spot);
        }
        // ogon: metallic gold
        case 3: { return mix(vec3<f32>(0.95, 0.68, 0.18), vec3<f32>(1.0, 0.9, 0.55), smoothstep(0.3, 0.75, n)); }
        // asagi: blue-grey with a net of scales, red along the sides
        case 4: {
            let sc = vec2<f32>(u * len / 2.4, v * 3.0);
            let netl = smoothstep(0.08, 0.0, min(abs(fract(sc.x + 0.5 * floor(sc.y)) - 0.5), abs(fract(sc.y) - 0.5)) - 0.3);
            var c = mix(vec3<f32>(0.42, 0.52, 0.62), vec3<f32>(0.2, 0.28, 0.38), netl * 0.6);
            return mix(c, red * 0.95, smoothstep(0.55, 0.9, abs(v)));
        }
        // chagoi: tea brown, each scale outlined
        default: { return mix(vec3<f32>(0.5, 0.33, 0.18), vec3<f32>(0.68, 0.48, 0.28), n); }
    }
}
// --- the pond in the square: lily pads (x, y, radius, turn) and lotus flowers (x, y, radius)
const PADS = array<vec4<f32>, 12>(
    vec4<f32>(140.0, 142.0, 26.0, 0.4), vec4<f32>(340.0, 238.0, 24.0, 2.1), vec4<f32>(232.0, 344.0, 27.0, 4.0),
    vec4<f32>(36.0, 236.0, 25.0, 1.2), vec4<f32>(446.0, 150.0, 23.0, 5.1), vec4<f32>(440.0, 344.0, 19.0, 3.3),
    vec4<f32>(144.0, 442.0, 25.0, 0.9), vec4<f32>(338.0, 40.0, 24.0, 2.8), vec4<f32>(178.0, 120.0, 13.0, 3.7),
    vec4<f32>(196.0, 372.0, 12.0, 5.6), vec4<f32>(70.0, 266.0, 12.0, 1.9), vec4<f32>(372.0, 68.0, 12.0, 4.4),
);
const LOTUS = array<vec3<f32>, 5>(
    vec3<f32>(150.0, 136.0, 13.0), vec3<f32>(240.0, 350.0, 14.0), vec3<f32>(446.0, 146.0, 12.0), vec3<f32>(336.0, 36.0, 12.0), vec3<f32>(30.0, 232.0, 11.0),
);
// a lily pad round c (radius r, its notch turned by a), bobbing a little: .x distance, .yzw colour
fn pad(w: vec2<f32>, c: vec2<f32>, r: f32, a: f32, t: f32, rock: f32) -> vec4<f32> {
    // a slow bob, and a rocking when a koi passes under it or a ring reaches it
    let wob = vec2<f32>(sin(t * 0.4 + a), cos(t * 0.33 + a)) * 1.2 + vec2<f32>(sin(t * 6.0 + a), cos(t * 5.3 + a)) * 1.6 * rock;
    let d0 = rot(w - c - wob, a + 0.09 * rock * sin(t * 4.7 + a));
    let rr = length(d0);
    if (rr > r + 6.0) { return vec4<f32>(1e3, 0.0, 0.0, 0.0); }
    let ang = atan2(d0.y, d0.x);
    // the notch: a narrow wedge from the rim to the middle
    let notch = abs(d0.y) - d0.x * 0.14 - 0.5;
    var d = rr - r * (1.0 + 0.02 * sin(ang * 7.0 + a));
    d = max(d, -select(1e3, notch, d0.x > 0.0));
    // green, lighter towards the rim that turns up, veins from the middle
    var col = mix(vec3<f32>(0.1, 0.27, 0.09), vec3<f32>(0.24, 0.45, 0.14), smoothstep(0.0, r, rr) * 0.6 + 0.3 * vnoise(d0 * 0.12 + a * 3.0));
    let vein = smoothstep(0.12, 0.0, abs(fract(ang * 2.86) - 0.5) * 2.0 * rr * 0.12 - 0.02) * smoothstep(2.0, 6.0, rr);
    col *= 1.0 - 0.18 * vein;
    // the rim turned up: a bright edge on the light's side (upper left), a dark one opposite
    let lightside = dot(normalize(d0 + vec2<f32>(0.001)), rot(normalize(vec2<f32>(-0.7, -0.7)), a));
    col = mix(col, vec3<f32>(0.46, 0.62, 0.26), smoothstep(-2.6, -0.4, d) * clamp(lightside, 0.0, 1.0) * 0.7);
    col = mix(col, vec3<f32>(0.05, 0.13, 0.05), smoothstep(-2.0, -0.2, d) * clamp(-lightside, 0.0, 1.0) * 0.6);
    // a few drops of water: bright specks with a dark lower lip
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let dp = vec2<f32>(hash(vec2<f32>(a, fk)) - 0.5, hash(vec2<f32>(fk, a)) - 0.5) * r * 1.1;
        let dq = d0 - dp;
        let dr = length(dq) - (0.9 + 0.7 * hash(vec2<f32>(fk + a, 3.0)));
        if (dr < 0.6) {
            col = mix(col, col * 0.55, smoothstep(0.6, -0.4, dr));
            col = mix(col, vec3<f32>(0.85, 0.95, 0.9), smoothstep(0.5, -0.3, length(dq + vec2<f32>(0.35, 0.35)) - 0.45));
        }
    }
    return vec4<f32>(d, col);
}
// a lotus flower seen from above: an outer ring of eight petals, an inner of six, a golden seed head
fn lotus(w: vec2<f32>, c: vec2<f32>, r: f32, t: f32) -> vec4<f32> {
    let d0 = rot(w - c, t * 0.03 + c.x);
    let rr = length(d0);
    if (rr > r * 1.2) { return vec4<f32>(1e3, 0.0, 0.0, 0.0); }
    let ang = atan2(d0.y, d0.x);
    var best = vec4<f32>(1e3, 0.0, 0.0, 0.0);
    for (var ring = 0; ring < 2; ring++) {
        let n = select(8.0, 6.0, ring == 1);
        let len = select(r, r * 0.62, ring == 1);
        let off = select(0.0, 0.5, ring == 1);
        let sector = 6.2831853 / n;
        let k = floor(ang / sector - off + 0.5) + off;
        for (var j = -1; j <= 1; j++) {
            let pa = (k + f32(j)) * sector;
            let u = rot(d0, pa);
            let e = (u - vec2<f32>(len * 0.52, 0.0)) / vec2<f32>(len * 0.5, len * 0.22);
            let dp = (length(e) - 1.0) * len * 0.22;
            // the inner ring lies over the outer
            let dd = select(dp, dp - 0.01, ring == 1);
            if (dd < best.x || (ring == 1 && dp < 0.3)) {
                let along = clamp(u.x / len, 0.0, 1.0);
                var pc = mix(vec3<f32>(0.86, 0.3, 0.5), vec3<f32>(1.0, 0.86, 0.92), smoothstep(0.2, 0.95, along));
                pc *= 0.85 + 0.15 * smoothstep(1.0, 0.0, abs(e.y));
                pc *= select(0.92, 1.0, ring == 1);
                best = vec4<f32>(dp, pc);
            }
        }
    }
    let seedh = rr - r * 0.2;
    if (seedh < best.x || seedh < 0.0) {
        var sc = vec3<f32>(0.9, 0.78, 0.3);
        sc *= 1.0 - 0.35 * step(0.7, hash(floor(d0 * 1.2)));
        best = vec4<f32>(seedh, sc);
    }
    return best;
}
// the stones round the pond: a ring of rounded rocks at radius ~232 from the square's middle, moss
// beyond; .x distance to the rocks (negative inside one), .y 1 beyond the water, .zw the rock's light
fn stones(w: vec2<f32>) -> vec4<f32> {
    let rel = w - vec2<f32>(240.0);
    let rr = length(rel);
    if (rr < 200.0) { return vec4<f32>(1e3, 0.0, 0.0, 0.0); }
    let ang = atan2(rel.y, rel.x);
    var best = vec4<f32>(1e3, 0.0, 0.0, 0.0);
    for (var ring = 0; ring < 2; ring++) {
        let rad = select(228.0, 252.0, ring == 1);
        let n = select(52.0, 58.0, ring == 1);
        let sector = 6.2831853 / n;
        let k0 = floor(ang / sector + f32(ring) * 0.5);
        for (var j = -1; j <= 1; j++) {
            let k = k0 + f32(j) + f32(ring) * 0.5;
            let h = hash(vec2<f32>(k, f32(ring) + 3.0));
            let a = k * sector;
            let cc = vec2<f32>(240.0) + vec2<f32>(cos(a), sin(a)) * (rad + (h - 0.5) * 8.0);
            let sr = 11.0 + 7.0 * hash(vec2<f32>(f32(ring), k));
            let q = rot(w - cc, h * 6.28);
            let ds = (length(q / vec2<f32>(1.0, 0.72 + 0.2 * h)) - sr) * 0.8 + 1.4 * (vnoise(w * 0.2 + h * 9.0) - 0.5);
            if (ds < best.x) {
                // lit from the upper left: a rounded top
                let nrm = (w - cc) / sr;
                let lit = clamp(0.55 - dot(nrm, vec2<f32>(0.6, 0.6)) * 0.5, 0.0, 1.0) * smoothstep(0.0, -4.0, ds);
                best = vec4<f32>(ds, step(rad + 2.0, rr), lit, h);
            }
        }
    }
    return best;
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let pp = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let w = world(pp);
    let v = to_view(pp);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time;
    // --- the bottom: rounded pebbles under deep green water, deeper towards the pond's middle
    let bend = vec2<f32>(vnoise(w * 0.05 + vec2<f32>(t * 0.2, 0.0)), vnoise(w * 0.05 + vec2<f32>(0.0, t * 0.17) + 7.0)) * 3.0;
    let pq = (w + bend) / 11.0;
    let pc = floor(pq);
    var near = 9.0;
    var second = 9.0;
    var stone = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let g = pc + vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(hash(g + 0.3), hash(g + 5.1)) * 0.8 + 0.1;
            let dd = length(pq - g - o);
            if (dd < near) { second = near; near = dd; stone = hash(g + 9.7); } else if (dd < second) { second = dd; }
        }
    }
    let edge = second - near;
    let bottom = mix(vec3<f32>(0.24, 0.24, 0.2), vec3<f32>(0.38, 0.32, 0.22), stone) * (0.55 + 0.45 * smoothstep(0.0, 0.5, edge)) * (0.85 + 0.25 * smoothstep(0.6, 0.0, near));
    let deep = smoothstep(260.0, 40.0, length(w - vec2<f32>(240.0)));
    var water = mix(bottom * vec3<f32>(0.58, 0.8, 0.66), vec3<f32>(0.02, 0.12, 0.11), 0.3 + 0.38 * deep);
    water = mix(water, vec3<f32>(0.04, 0.19, 0.16), vnoise(w * 0.02 + vec2<f32>(t * 0.03, 0.0)) * 0.3);
    // the sun's net of light on the bottom, brighter where the waves gather
    let w1 = vnoise(w * 0.07 + vec2<f32>(t * 0.35, t * 0.2));
    let w2 = vnoise(w * 0.07 + vec2<f32>(-t * 0.25, t * 0.3) + 13.0);
    let gather = smoothstep(0.3, 0.8, vnoise(w * 0.025 + vec2<f32>(t * 0.08, -t * 0.05)));
    let caustic = smoothstep(0.78 - 0.1 * gather, 1.0, 1.0 - abs(w1 - w2) * 2.2) * (0.35 + 0.65 * gather);
    water += vec3<f32>(0.2, 0.28, 0.18) * caustic * (1.0 - 0.55 * deep);
    // the sky in the surface: a soft pale band from the upper left, a cloud drifting through it
    let sky = smoothstep(0.62, 0.0, (w.x + w.y * 1.2) / 1056.0) * (0.5 + 0.5 * smoothstep(0.45, 0.7, fbm(w * 0.008 + vec2<f32>(t * 0.01, 0.0))));
    water = mix(water, vec3<f32>(0.42, 0.55, 0.6), sky * 0.22);
    var col = water;
    // the koi's places this frame, in the square's coordinates (the petals and the pads feel them)
    let off = (vec2<f32>(480.0) - view_size()) * 0.5;
    var fishw = array<vec2<f32>, 6>(vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(0.0));
    for (var j = 0; j < 6; j++) {
        fishw[j] = place(f32(j), t) + off;
    }
    // --- the koi's shadows on the bottom, then the koi (in the view as it stands)
    for (var j = 0; j < 6; j++) {
        let fj = f32(j);
        let at = place(fj, t);
        let ahead = place(fj, t + 0.15);
        let heading = normalize(ahead - at + vec2<f32>(0.0001, 0.0));
        let len = 34.0 + 9.0 * hash(vec2<f32>(fj, 5.0));
        let d0 = v - at - vec2<f32>(6.0, 8.0);
        if (dot(d0, d0) < len * len * 1.1) {
            let q = vec2<f32>(-dot(d0, heading), d0.x * heading.y - d0.y * heading.x);
            let s = koi(q, len, t * 5.0 + fj);
            col *= 1.0 - 0.38 * smoothstep(3.5, -2.5, min(s.x, s.y));
        }
    }
    for (var j = 0; j < 6; j++) {
        let fj = f32(j);
        let at = place(fj, t);
        let ahead = place(fj, t + 0.15);
        let heading = normalize(ahead - at + vec2<f32>(0.0001, 0.0));
        let len = 34.0 + 9.0 * hash(vec2<f32>(fj, 5.0));
        let d0 = v - at;
        if (dot(d0, d0) > len * len * 1.1) { continue; }
        let q = vec2<f32>(-dot(d0, heading), d0.x * heading.y - d0.y * heading.x);
        let speed = length(ahead - at) / 0.15;
        let s = koi(q, len, t * (4.0 + speed * 0.08) + fj);
        // fins and the veil tail: see-through, streaked, the colour of the body faded into the water
        if (s.y < 0.8) {
            // fine rays fanning from the root, the fin's own colour a paler body colour
            let streak = 0.88 + 0.12 * sin(atan2(q.y, q.x - len * 0.35) * 30.0);
            let fc = mix(skin(j, 0.7, 0.0, len, fj), vec3<f32>(0.96, 0.94, 0.9), 0.3) * streak;
            col = mix(col, fc, smoothstep(1.4, -1.6, s.y) * 0.5);
        }
        if (s.x < 0.8) {
            var fish = skin(j, s.w, s.z, len, fj);
            // rounded back: lighter down the spine, the sides in shade; a sheen along the spine
            fish *= 0.62 + 0.42 * (1.0 - s.z * s.z);
            fish += vec3<f32>(0.22, 0.22, 0.2) * exp(-s.z * s.z * 14.0) * smoothstep(0.12, 0.3, s.w) * (1.0 - smoothstep(0.75, 0.95, s.w));
            // scales: a fine net across the back
            let sc = vec2<f32>(s.w * len / 2.0, s.z * 2.6);
            let net = min(abs(fract(sc.x + 0.5 * floor(sc.y)) - 0.5), abs(fract(sc.y) - 0.5));
            fish *= 0.93 + 0.07 * smoothstep(0.0, 0.18, net);
            // the eyes, set wide on the head
            let eye = min(length(q - vec2<f32>(-len * 0.4, len * 0.075)), length(q - vec2<f32>(-len * 0.4, -len * 0.075))) - 1.1;
            fish = mix(fish, vec3<f32>(0.04, 0.04, 0.05), smoothstep(0.5, -0.5, eye));
            col = mix(col, fish, smoothstep(0.7, -0.7, s.x));
        }
    }
    // --- food after a press: pellets sinking, rings spreading over the water
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = t - b.z;
        if (age < 0.0 || age > 3.5) { continue; }
        let fp = to_view(b.xy);
        for (var k = 0; k < 3; k++) {
            let ring = abs(length(v - fp) - age * (30.0 + 9.0 * f32(k)) + f32(k) * 7.0);
            col += vec3<f32>(0.55, 0.7, 0.66) * smoothstep(1.6, 0.0, ring) * (1.0 - age / 3.5) * 0.45 * step(f32(k) * 0.25, age);
        }
        for (var k = 0; k < 7; k++) {
            let fk = f32(k);
            let pel = fp + vec2<f32>(hash(vec2<f32>(fk, b.z)) - 0.5, hash(vec2<f32>(b.z, fk)) - 0.5) * 18.0 * smoothstep(0.0, 0.5, age);
            let pd = length(v - pel) - 1.3;
            col = mix(col, vec3<f32>(0.5, 0.34, 0.16) * (1.0 - 0.5 * smoothstep(1.0, 3.0, age)), smoothstep(0.6, -0.4, pd) * (1.0 - smoothstep(2.5, 3.5, age)));
        }
    }
    // --- the lily pads and lotus flowers floating on top, their soft shadows on the water
    for (var i = 0; i < 12; i++) {
        let pd = PADS[i];
        if (abs(w.x - pd.x) > pd.z + 10.0 || abs(w.y - pd.y) > pd.z + 10.0) { continue; }
        let sh = length(w - pd.xy - vec2<f32>(3.0, 4.0)) - pd.z;
        col *= 1.0 - 0.32 * smoothstep(3.0, -2.0, sh);
        var rock = 0.0;
        for (var j = 0; j < 6; j++) {
            rock += exp(-max(length(fishw[j] - pd.xy) - pd.z * 0.6, 0.0) / 10.0);
        }
        for (var k = 0; k < 3; k++) {
            let b = f.aux[k];
            if (b.w < 0.5) { continue; }
            let age = t - b.z;
            let rd = length(pd.xy - (to_view(b.xy) + off));
            rock += exp(-abs(rd - age * 34.0) / 14.0) * (1.0 - smoothstep(2.0, 4.0, age)) * step(0.0, age);
        }
        let pl = pad(w, pd.xy, pd.z, pd.w, t, min(rock, 1.5));
        col = mix(col, pl.yzw, smoothstep(0.7, -0.7, pl.x));
    }
    for (var i = 0; i < 5; i++) {
        let lo = LOTUS[i];
        if (abs(w.x - lo.x) > lo.z * 1.4 || abs(w.y - lo.y) > lo.z * 1.4) { continue; }
        let sh = length(w - lo.xy - vec2<f32>(2.0, 3.0)) - lo.z * 0.9;
        col *= 1.0 - 0.25 * smoothstep(2.0, -2.0, sh);
        let fl = lotus(w, lo.xy, lo.z, t);
        col = mix(col, fl.yzw, smoothstep(0.6, -0.6, fl.x));
    }
    // --- the stones round the pond and the moss beyond (only in the version with stones)
    if (STONES) {
        let st = stones(w);
        let rr = length(w - vec2<f32>(240.0));
        // the moss beyond the rocks, the water darker in their shade
        let moss = smoothstep(236.0, 246.0, rr);
        col = mix(col, mix(vec3<f32>(0.08, 0.17, 0.06), vec3<f32>(0.2, 0.32, 0.1), vnoise(w * 0.18)) * (0.8 + 0.3 * vnoise(w * 0.6)), moss);
        col *= 1.0 - 0.3 * smoothstep(12.0, 0.0, st.x) * (1.0 - moss);
        if (st.x < 1.0) {
            var rc = mix(vec3<f32>(0.2, 0.2, 0.21), vec3<f32>(0.55, 0.54, 0.5), st.z);
            rc *= 0.85 + 0.25 * vnoise(w * 0.5 + st.w * 7.0);
            // moss on the rocks' tops
            rc = mix(rc, vec3<f32>(0.22, 0.34, 0.1), smoothstep(0.55, 0.75, vnoise(w * 0.15 + st.w * 3.0)) * 0.7);
            col = mix(col, rc, smoothstep(0.7, -0.7, st.x));
        }
    }
    c = mix(c, col, f.strength / 0.6 * 0.95 * (1.0 - keep_bg(c)));
    put(id.xy, clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
}
