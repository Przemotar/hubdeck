// B5: the icons as objects. From the content's cover a height is made (the cover softened over a
// few pixels: an icon's middle stands higher than its rim), its slope gives each pixel a normal,
// and the deck's light (`aux[15]`, the lamp's when it is on) lights it as a material
// (`aux[0].x`): 0 relief (the icon's own colours, raised and lit), 1 gold, 2 chrome, 3 crystal.
// Its `mask` is the icons' cover without the labels (`icons_only`).
// No icon is modelled: every app's own logo becomes the object.
fn soft_cover(p: vec2<f32>) -> f32 {
    // the cover over a small disc: 13 taps
    var s = cover(i32(p.x), i32(p.y)) * 2.0;
    let r1 = 1.6;
    let r2 = 3.2;
    for (var i = 0; i < 6; i++) {
        let a = f32(i) * 1.0472;
        let d = vec2<f32>(cos(a), sin(a));
        s += cover(i32(round(p.x + d.x * r1)), i32(round(p.y + d.y * r1))) * 1.5;
        s += cover(i32(round(p.x + d.x * r2 + d.y)), i32(round(p.y + d.y * r2 - d.x))) * 1.0;
    }
    return s / 17.0;
}
fn light_at(p: vec2<f32>) -> vec3<f32> {
    if (f.aux[15].w > 0.0) { return f.aux[15].xyz; }
    let size = vec2<f32>(f.size);
    let a = f.time * 0.16 + f.wind * 3.14159;
    return vec3<f32>(size.x * (0.5 + 0.62 * cos(a)), -30.0 + 22.0 * sin(a * 1.3), 110.0);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    var m = cover(i32(id.x), i32(id.y));
    // the cover here is the icons' alone (the engine leaves labels and values out); a picture
    // filling the whole key (`aux_at(16 + k)` 1, found by the engine) stays as it is
    let k = key_at(p);
    var objects = 0.0;
    if (k >= 0) {
        objects = 1.0 - aux_at(16 + k);
    }
    m *= objects;
    let l3s = light_at(p);
    if (m < 0.02) {
        // the icon stands off its key: its soft shadow falls away from the light
        if (objects > 0.0) {
            let away = normalize(p - l3s.xy + vec2<f32>(0.001)) * 4.5;
            let shade = soft_cover(p - away) * objects;
            c *= 1.0 - 0.78 * shade * f.strength;
        }
        put(id.xy, c);
        return;
    }
    // height and its slope
    let hx = soft_cover(p + vec2<f32>(1.0, 0.0)) - soft_cover(p - vec2<f32>(1.0, 0.0));
    let hy = soft_cover(p + vec2<f32>(0.0, 1.0)) - soft_cover(p - vec2<f32>(0.0, 1.0));
    // a strong relief: on a key 14 mm wide a gentle one reads as a colour, not a shape
    let n = normalize(vec3<f32>(-hx * 6.5, -hy * 6.5, 1.0));
    let l3 = light_at(p);
    let l = normalize(vec3<f32>(l3.xy - p, l3.z));
    let v = vec3<f32>(0.0, 0.0, 1.0);
    let h = normalize(l + v);
    let diffuse = clamp(dot(n, l), 0.0, 1.0);
    let spec_soft = pow(clamp(dot(n, h), 0.0, 1.0), 18.0);
    let spec_hard = pow(clamp(dot(n, h), 0.0, 1.0), 90.0);
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    let kind = i32(f.aux[0].x + 0.5);
    var o = c;
    if (kind == 0) {
        // relief: its own colours, lit and shaded, a highlight on the raised rim
        o = c * (0.25 + 1.05 * diffuse) + vec3<f32>(1.0) * (spec_soft * 0.6 + spec_hard * 0.9);
    } else if (kind == 1) {
        // gold: the logo's light and dark kept as the metal's depth, warm reflections
        // the logo's shapes kept in the metal: its light and dark stretched across bronze to bright gold
        let shade = smoothstep(0.08, 0.85, luma);
        let base = mix(vec3<f32>(0.50, 0.32, 0.08), vec3<f32>(1.0, 0.86, 0.48), 0.25 + 0.75 * shade);
        // polished metal: a dark band and a bright band of the room reflected, bent by the relief
        let r = reflect(-v, n);
        let env = mix(vec3<f32>(0.30, 0.17, 0.04), vec3<f32>(1.0, 0.93, 0.68), smoothstep(-0.35, 0.25, -r.y + 0.3 * r.x));
        o = base * (0.35 + 0.65 * diffuse) * env * 1.35 + vec3<f32>(1.0, 0.95, 0.75) * (spec_soft * 0.7 + spec_hard * 1.3);
    } else if (kind == 2) {
        // chrome: a mirror of a horizon (bright sky over a dark ground) bent by the relief
        let r = reflect(-v, n);
        let sky = mix(vec3<f32>(0.05, 0.06, 0.08), vec3<f32>(0.95, 0.97, 1.0), smoothstep(-0.2, 0.2, -r.y + 0.15 * sin(f.time * 0.3)));
        o = mix(sky * (0.65 + 0.35 * luma), c, 0.08) + vec3<f32>(1.0) * (spec_hard * 1.6 + spec_soft * 0.3);
    } else {
        // crystal: the background seen through the icon, bent and tinted by its colour, a bright
        // rim and a glint
        let bent = px_smooth(p + vec2<f32>(hx, hy) * -14.0);
        o = mix(bent * 0.85 + c * 0.35, c, 0.25) + vec3<f32>(0.9, 0.97, 1.0) * (pow(1.0 - n.z, 1.5) * 0.9 + spec_hard * 1.1);
    }
    put(id.xy, mix(c, o, m * f.strength));
}
