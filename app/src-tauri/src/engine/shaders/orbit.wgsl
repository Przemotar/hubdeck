// Space B, in orbit: the Earth's curved limb across the bottom of the deck seen from a station,
// one view through the fifteen windows. Above it black space with stars; on the limb the thin
// blue line of the atmosphere, the sun rising over the edge on the left with its flare and a warm
// band of dawn; the planet's night side below with clouds faintly lit and the gold dust of city
// lights, an aurora's green curtain wavering over the limb on the right. A press: a scanner ping
// from the key, a ring sweeping out with a HUD tick where it passes (`aux[0..3]`: x, y, start, 1).
const EARTH_C = vec2<f32>(250.0, 1180.0);
const EARTH_R = 1000.0;
const SUN = vec2<f32>(70.0, 176.0);
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
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time * f.speed;
    let q = p - EARTH_C;
    let r = length(q);
    let h = r - EARTH_R;
    var col = vec3<f32>(0.003, 0.004, 0.012);
    // stars, fewer near the bright limb
    let sc = floor(p / 4.0);
    let st = hash(sc + 3.3);
    if (st > 0.975 && h > 6.0) {
        col += vec3<f32>(0.85, 0.9, 1.0) * smoothstep(1.4, 0.3, length(p - (sc + vec2<f32>(hash(sc), hash(sc + 1.0))) * 4.0)) * (0.5 + 0.5 * st) * smoothstep(6.0, 40.0, h);
    }
    // the sunrise: a white-gold sun just over the limb, its glow along it, a flare streak
    let ds = length(p - SUN);
    let along_limb = exp(-abs(h) / 9.0) * exp(-abs(p.x - SUN.x) / 160.0);
    col += vec3<f32>(1.0, 0.75, 0.42) * along_limb * 0.9;
    col += vec3<f32>(1.0, 0.92, 0.8) * exp(-ds / 7.0) * 1.2 + vec3<f32>(1.0, 0.7, 0.4) * exp(-ds / 40.0) * 0.35;
    col += vec3<f32>(0.6, 0.75, 1.0) * exp(-abs(p.y - SUN.y) / 1.5) * exp(-abs(p.x - SUN.x) / 120.0) * 0.4;
    // the atmosphere: a thin glowing blue line over the limb, wider towards the sun
    let glow = exp(-abs(h - 1.5) / (2.2 + 4.0 * exp(-abs(p.x - SUN.x) / 150.0)));
    col += mix(vec3<f32>(0.15, 0.4, 1.0), vec3<f32>(1.0, 0.6, 0.35), exp(-abs(p.x - SUN.x) / 90.0)) * glow * 0.85 * select(1.0, 0.6, h < 0.0);
    // the aurora over the night side on the right: green curtains with a red top, wavering
    if (p.x > 250.0 && h > -2.0 && h < 60.0) {
        // the curtain folds sway; inside them fine vertical rays shimmer upwards
        let wave = fbm(vec2<f32>(p.x * 0.012 + t * 0.04, t * 0.08));
        let fold = smoothstep(0.3, 0.7, vnoise(vec2<f32>(p.x * 0.035 + wave * 4.0, 0.5 + t * 0.05)));
        let rays = 0.55 + 0.45 * vnoise(vec2<f32>(p.x * 0.45 + wave * 9.0, h * 0.04 - t * 0.4));
        let height = exp(-max(h, 0.0) / 24.0) * smoothstep(-2.0, 4.0, h) * smoothstep(70.0, 20.0, h);
        let curtain = fold * rays * height * smoothstep(250.0, 340.0, p.x);
        col += mix(vec3<f32>(0.1, 0.95, 0.45), vec3<f32>(0.85, 0.2, 0.45), smoothstep(14.0, 50.0, h)) * curtain * 0.55;
    }
    // the planet: the night side, clouds lit faintly by the dawn, the gold of cities
    if (h < 0.8) {
        let n = q / EARTH_R;
        let ground = vec2<f32>(p.x + t * 1.8, p.y * 3.2);
        let clouds = smoothstep(0.5, 0.75, fbm(ground * 0.03));
        let land = smoothstep(0.48, 0.55, fbm(ground * 0.012 + 40.0));
        var e = mix(vec3<f32>(0.012, 0.025, 0.055), vec3<f32>(0.02, 0.026, 0.022), land);
        // dawn creeping in from the sun's side
        let dawn = exp(-abs(p.x - SUN.x) / 110.0) * exp(h / 30.0);
        e += vec3<f32>(0.5, 0.35, 0.25) * dawn * 0.35 * (0.4 + clouds);
        // clouds in the moonlight, brighter towards the dawn
        e = mix(e, vec3<f32>(0.11, 0.13, 0.17) * (0.55 + dawn), clouds * 0.75);
        // city lights on land under no cloud: gold dust in clusters
        // cities: round points of light in clusters (a town's core bright, its edge sparse), seen
        // a little foreshortened, never in rows
        let city = vec2<f32>(p.x + t * 1.8, p.y * 1.8);
        let cluster = smoothstep(0.52, 0.78, fbm(city * 0.03 + 70.0));
        for (var j = 0; j < 2; j++) {
            let cell = 3.0 + f32(j) * 2.3;
            let cg = floor(city / cell);
            let ch = hash(cg + 1.9 + f32(j) * 5.0);
            if (ch > 0.86 - 0.5 * cluster && land > 0.5) {
                let cp = (cg + vec2<f32>(0.2 + 0.6 * hash(cg + f32(j)), 0.2 + 0.6 * hash(cg + 3.0))) * cell;
                let glow = exp(-dot(city - cp, city - cp) / (0.45 + 0.6 * cluster));
                e += vec3<f32>(1.0, 0.74, 0.38) * glow * (0.3 + 0.9 * cluster) * (1.0 - clouds * 0.8) * (1.0 - dawn) * 0.8;
            }
        }
        col = mix(col, e, smoothstep(0.8, -0.8, h));
    }
    // a press: a scanner ping, a ring sweeping out, small ticks on it
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 1.8) { continue; }
        let d = length(p - b.xy);
        let ringd = abs(d - age * 220.0);
        let ang = atan2(p.y - b.y, p.x - b.x);
        let tick = step(0.85, fract(ang * 12.0 / 6.283)) * smoothstep(5.0, 2.0, ringd);
        col += vec3<f32>(0.4, 0.9, 1.0) * (exp(-ringd / 1.6) + tick * 0.6) * (1.0 - age / 1.8) * 0.7;
    }
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    if (lum > 0.45) { col *= (0.45 + (lum - 0.45) * 0.4) / lum; }
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
