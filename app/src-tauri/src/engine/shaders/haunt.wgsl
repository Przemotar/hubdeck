// Halloween, the haunted night: a landscape of black silhouettes behind the icons, across the whole
// deck as one view through the fifteen windows: rolling hills along the bottom, a crooked haunted
// house with lit windows that flicker, bare trees with twisted branches, gravestones and a fence;
// the frames between the keys hide parts of it as a real window frame would.
fn tree(p: vec2<f32>, base: vec2<f32>, h: f32, seed: f32) -> f32 {
    // a trunk leaning a little, and branches forking up and out
    let q = p - base;
    let lean = (hash(vec2<f32>(seed, 1.0)) - 0.5) * 0.3;
    let tx = q.x - q.y * lean;
    var d = max(abs(tx) - (2.2 - 1.4 * clamp(-q.y / h, 0.0, 1.0)), max(q.y, -q.y - h));
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let y0 = -h * (0.35 + 0.12 * fi);
        let dir = select(-1.0, 1.0, hash(vec2<f32>(seed, fi)) > 0.5);
        let len = h * (0.35 - 0.04 * fi);
        let b0 = vec2<f32>(lean * -y0, y0);
        let b1 = b0 + vec2<f32>(dir * len, -len * 0.55);
        let ab = b1 - b0;
        let t = clamp(dot(q - b0, ab) / dot(ab, ab), 0.0, 1.0);
        d = min(d, length(q - (b0 + ab * t)) - mix(1.4, 0.4, t));
        // a twig off the branch
        let c1 = b0 + ab * 0.6;
        let c2 = c1 + vec2<f32>(dir * len * 0.3, -len * 0.45);
        let cd = c2 - c1;
        let u = clamp(dot(q - c1, cd) / dot(cd, cd), 0.0, 1.0);
        d = min(d, length(q - (c1 + cd * u)) - 0.6);
    }
    return d;
}
fn house(p: vec2<f32>, base: vec2<f32>, s: f32) -> vec2<f32> {
    // walls, a steep crooked roof, a tower; returns the silhouette's distance and its windows
    let q = (p - base) / s;
    let walls = max(abs(q.x) - 1.0, max(q.y, -q.y - 1.3));
    let roof = max(abs(q.x) * 1.15 + (q.y + 1.3) * 1.0 - 0.0 + q.x * 0.12, -(q.y + 2.6));
    let tower = max(abs(q.x - 0.65) - 0.32, max(q.y + 1.0, -q.y - 2.7));
    let spire = max(abs(q.x - 0.65) * 2.2 + (q.y + 2.7) - 0.0, -(q.y + 3.5));
    let d = min(min(walls, max(roof, q.y + 1.2)), min(tower, spire)) * s;
    // windows: small rectangles, lit
    let w1 = max(abs(q.x + 0.45) - 0.18, abs(q.y + 0.75) - 0.22);
    let w2 = max(abs(q.x - 0.15) - 0.18, abs(q.y + 0.75) - 0.22);
    let w3 = max(abs(q.x - 0.65) - 0.1, abs(q.y + 2.0) - 0.16);
    let win = min(min(w1, w2), w3) * s;
    return vec2<f32>(d, win);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    // hills: two layers, the far one paler with the night
    let far_h = size.y - 58.0 - 14.0 * sin(p.x * 0.012 + 1.3) - 8.0 * sin(p.x * 0.031);
    let near_h = size.y - 34.0 - 10.0 * sin(p.x * 0.017 + 0.4) - 6.0 * sin(p.x * 0.043 + 2.0);
    let far = smoothstep(0.8, -0.8, far_h - p.y);
    // the house on the far hill, right of the middle
    let hb = vec2<f32>(size.x * 0.66, size.y - 66.0);
    let hs = house(p, hb, 18.0);
    // the near hill (negative inside), trees, gravestones and a fence on it
    var sil = near_h - p.y;
    sil = min(sil, tree(p, vec2<f32>(size.x * 0.18, near_h + 2.0), 74.0, 3.0));
    sil = min(sil, tree(p, vec2<f32>(size.x * 0.88, near_h + 2.0), 62.0, 7.0));
    sil = min(sil, tree(p, vec2<f32>(size.x * 0.40, far_h + 2.0), 46.0, 11.0));
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let gx = size.x * (0.27 + 0.11 * fi) + 6.0 * hash(vec2<f32>(fi, 5.0));
        let gy = size.y - 34.0 - 10.0 * sin(gx * 0.017 + 0.4) - 6.0 * sin(gx * 0.043 + 2.0);
        let g = p - vec2<f32>(gx, gy - 7.0);
        let stone = max(abs(g.x) - 4.0, max(g.y - 7.0, -g.y - 5.0));
        let top = length(g - vec2<f32>(0.0, -5.0)) - 4.0;
        sil = min(sil, min(stone, max(top, g.y + 5.0)));
    }
    // fence posts along the near hill on the right
    let fx = fract((p.x - size.x * 0.72) / 9.0) * 9.0 - 4.5;
    let post = max(abs(fx) - 1.0, (near_h - 15.0) - p.y);
    let rail = max(abs(p.y - (near_h - 10.0)) - 0.7, 0.0);
    if (p.x > size.x * 0.72) { sil = min(sil, min(post, rail + step(p.x, size.x * 0.72) * 1e3)); }
    let ink = vec3<f32>(0.035, 0.025, 0.06);
    // the far hill and the house, a shade lighter (night air)
    c = mix(c, mix(c, vec3<f32>(0.09, 0.06, 0.14), 0.85), far * f.strength * (1.0 - keep_bg(c)));
    c = mix(c, vec3<f32>(0.07, 0.05, 0.11), smoothstep(0.8, -0.8, hs.x) * f.strength * (1.0 - keep_bg(c)));
    // its windows glow, flickering like candles
    let flick = 0.75 + 0.25 * vnoise(vec2<f32>(f.time * 6.0, 3.0));
    c = mix(c, vec3<f32>(1.0, 0.72, 0.25) * flick, smoothstep(0.6, -0.4, hs.y) * step(hs.x, 0.0) * f.strength);
    c += vec3<f32>(1.0, 0.6, 0.2) * exp(-max(hs.y, 0.0) / 3.0) * step(hs.x, 2.0) * 0.12 * flick * f.strength;
    // the near ground and everything on it: ink black with a faint rim of moonlight
    let inside = smoothstep(0.8, -0.8, -sil);
    let rim = smoothstep(1.6, 0.0, abs(sil)) * 0.18;
    c = mix(c, ink + vec3<f32>(0.25, 0.2, 0.4) * rim, inside * f.strength * (1.0 - keep_bg(c) * 0.5));
    put(id.xy, c);
}
