// K3 trial: an aquarium behind the fifteen windows, one world across the deck. Fish at three
// depths (near ones bigger, faster, sharper; far ones small, slow, sunk in blue), light from the
// surface (shafts and caustics), sand and weed at the bottom, bubbles. The deck's frames hide what
// swims behind them; the slow pan of the view moves near things more than far ones.
fn sd_fish(q: vec2<f32>, len: f32, h: f32, wag: f32) -> f32 {
    // q: fish space, x along the body (head at +len/2), y across
    let u = clamp(q.x / (len * 0.5), -1.0, 1.0);
    let bend = sin(wag - u * 2.4) * h * 0.18 * (1.0 - u) * 0.5;
    let y = q.y - bend;
    let body = length(vec2<f32>(q.x / (len * 0.5), y / (h * 0.5))) - 1.0;
    // the tail: a fan behind the body
    let tx = -(q.x + len * 0.5);
    let tail = select(1.0, max(abs(y) - tx * 0.75 - h * 0.08, tx - len * 0.32) / (h * 0.5), tx > -len * 0.12);
    return min(body, tail);
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let size = vec2<f32>(f.size);
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let base = px(i32(id.x), i32(id.y)).rgb;
    let t = f.time;
    let pan = sin(t * 0.07) * 14.0;
    let uv = p / size;
    // the water: lighter toward the surface, the tint at its heart
    // never darker than the screen can show (~#24): it crushes anything below
    let deep = mix(vec3<f32>(0.11, 0.38, 0.48), vec3<f32>(0.07, 0.22, 0.32), uv.y);
    var c = mix(deep, deep * 0.6 + f.tint.rgb * 0.25, 0.35);
    // shafts from the surface, swaying
    let sx = (p.x + pan * 0.3) / size.x + uv.y * 0.18;
    let shafts = pow(0.5 + 0.5 * sin(sx * 23.0 + sin(t * 0.3 + sx * 5.0) * 1.3), 6.0) * (1.0 - uv.y) * 0.18;
    c += vec3<f32>(0.55, 0.85, 0.9) * shafts;
    // caustics on everything behind, slowly
    let cp = (p + vec2<f32>(pan * 0.5, 0.0)) / 26.0;
    let w1 = vec2<f32>(vnoise(cp + vec2<f32>(t * 0.21, 0.0)), vnoise(cp + vec2<f32>(4.0, t * 0.17)));
    let cn = vnoise(cp * 1.7 + w1 * 2.2 + vec2<f32>(0.0, t * 0.1));
    let caustic = pow(1.0 - abs(cn * 2.0 - 1.0), 9.0);
    c += vec3<f32>(0.5, 0.9, 0.85) * caustic * 0.15;
    // sand with gentle dunes in the bottom row, weed swaying
    let floor_y = size.y - 18.0 + sin((p.x + pan) * 0.045) * 6.0 + sin((p.x + pan) * 0.11) * 2.0;
    if (p.y > floor_y) {
        let sand = vec3<f32>(0.55, 0.46, 0.30) * (0.55 + 0.25 * vnoise(p / 3.0)) * (0.8 + caustic * 0.9);
        c = mix(c, sand, smoothstep(floor_y, floor_y + 4.0, p.y) * 0.85);
    }
    for (var i = 0; i < 7; i++) {
        let fi = f32(i);
        let root = size.x * hash(vec2<f32>(fi, 11.0)) - pan * 0.8;
        let height = 26.0 + 30.0 * hash(vec2<f32>(fi, 12.0));
        let along = clamp((size.y - p.y) / height, 0.0, 1.0);
        let sway = sin(t * 0.8 + fi * 1.7 + along * 2.0) * along * along * 9.0;
        let d = abs(p.x - root - sway) - (1.6 - along * 1.1);
        if (p.y > size.y - height && d < 1.0) {
            let weed = mix(vec3<f32>(0.10, 0.35, 0.18), vec3<f32>(0.25, 0.6, 0.3), along);
            c = mix(c, weed, smoothstep(1.0, -0.5, d) * 0.9);
        }
    }
    // fish: far, middle, near
    for (var i = 0; i < 8; i++) {
        let fi = f32(i);
        let z = fract(fi * 0.37 + 0.11);
        let len = mix(12.0, 34.0, z * z);
        let h = len * mix(0.42, 0.55, hash(vec2<f32>(fi, 3.0)));
        let speed = mix(9.0, 26.0, z) * (0.8 + 0.4 * hash(vec2<f32>(fi, 4.0)));
        let dir = select(-1.0, 1.0, hash(vec2<f32>(fi, 5.0)) > 0.5);
        let span = size.x + 90.0;
        let x = fract((hash(vec2<f32>(fi, 6.0)) * span + dir * speed * t) / span) * span - 45.0 + pan * (0.3 + z);
        let y = mix(24.0, size.y - 40.0, hash(vec2<f32>(fi, 7.0))) + sin(t * 0.5 + fi * 2.1) * 8.0;
        var q = p - vec2<f32>(x, y);
        q.x *= dir;
        let d = sd_fish(q, len, h, t * (5.0 + 3.0 * z) + fi);
        if (d < 0.3) {
            let hue = hash(vec2<f32>(fi, 8.0));
            var fc = select(select(vec3<f32>(1.0, 0.55, 0.18), vec3<f32>(0.95, 0.85, 0.35), hue > 0.33), vec3<f32>(0.35, 0.7, 1.0), hue > 0.66);
            // lighter belly, a stripe, an eye
            let belly = smoothstep(-h * 0.1, h * 0.35, q.y);
            fc = mix(fc * 0.75, fc * 1.15 + 0.1, belly);
            if (hue <= 0.33) {
                fc = mix(fc, vec3<f32>(0.95), smoothstep(1.2, 0.4, abs(q.x - len * 0.08)) * step(d, 0.0));
            }
            let eye = distance(q, vec2<f32>(len * 0.3, -h * 0.08));
            fc = mix(fc, vec3<f32>(0.02), smoothstep(len * 0.055 + 0.5, len * 0.055 - 0.3, eye));
            // far fish sink into the water's blue
            fc = mix(c * 1.2 + vec3<f32>(0.02, 0.06, 0.08), fc, 0.35 + 0.65 * z);
            c = mix(c, fc, smoothstep(0.25, -0.12 - 0.25 * (1.0 - z), d));
        }
    }
    // bubbles from three places in the sand
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        let src = size.x * (0.15 + 0.35 * f32(i % 3)) + 20.0 * hash(vec2<f32>(fi, 20.0)) - pan * 0.8;
        let life = 6.0 + 3.0 * hash(vec2<f32>(fi, 21.0));
        let age = fract(t / life + hash(vec2<f32>(fi, 22.0)));
        let bp = vec2<f32>(src + sin(age * 18.0 + fi) * 3.0, size.y - 20.0 - age * (size.y + 10.0));
        let r = 1.0 + 1.6 * hash(vec2<f32>(fi, 23.0)) + age * 0.8;
        let bd = distance(p, bp);
        let ring = smoothstep(r + 0.6, r, bd) * (0.35 + 0.65 * smoothstep(r - 1.2, r, bd));
        c += vec3<f32>(0.7, 0.95, 1.0) * ring * 0.45;
        c += vec3<f32>(1.0) * smoothstep(0.8, 0.0, distance(p, bp + vec2<f32>(-r * 0.35, -r * 0.35))) * 0.5;
    }
    put(id.xy, mix(base, c, (0.88 * f.strength + 0.12) * (1.0 - keep_bg(base))));
}
