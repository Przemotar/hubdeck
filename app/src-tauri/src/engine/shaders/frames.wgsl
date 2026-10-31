// K2 trial: the keys' frames as real walls above the screen. A lamp above the deck (drifting beyond
// its top edge) casts each frame's shadow into its window, the window's corners stay deeper, and
// light that passes the frames' edges leaks in as soft beams with dust in them. The key's own
// colours are kept: only light and shadow are added.
// the deck's one light (`aux[15]`, the lamp's when it is on), or a lamp of its own beyond the top
fn light_pos() -> vec3<f32> {
    if (f.aux[15].w > 0.0) { return f.aux[15].xyz; }
    let size = vec2<f32>(f.size);
    let a = f.time * 0.16 + f.wind * 3.14159;
    return vec3<f32>(size.x * (0.5 + 0.62 * cos(a)), -30.0 + 22.0 * sin(a * 1.3), 110.0);
}
fn inside(p: vec2<f32>, r: vec4<f32>) -> f32 {
    return min(min(p.x - r.x, r.x + r.z - p.x), min(p.y - r.y, r.y + r.w - p.y));
}
fn window_of(p: vec2<f32>) -> i32 {
    for (var k = 0; k < 15; k++) {
        if (inside(p, keys[k]) > 0.0) { return k; }
    }
    return -1;
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let k = window_of(p);
    if (k < 0) { put(id.xy, c); return; }
    let r = keys[k];
    let l = light_pos();
    let to = l.xy - p;
    let dist = length(to);
    let dir = to / max(dist, 0.001);
    // the wall's height above the screen, px: its shadow reaches as far as the lamp is low
    let wall = 11.0;
    let reach = wall * dist / l.z;
    let edge = inside(p + dir * reach, r);
    let soft = 0.7 + reach * 0.22;
    let lit = smoothstep(-soft, soft, edge);
    let ao = smoothstep(0.0, 6.0, inside(p, r));
    let s = f.strength;
    c *= mix(1.0 - 0.72 * s, 1.0, lit) * mix(1.0 - 0.35 * s, 1.0, ao);
    // the light that grazes past the lit wall's top edge: a soft band just inside the shadow line,
    // warmer and stronger near the lamp
    let near = 1.0 / (1.0 + dist * dist / (260.0 * 260.0));
    let graze = exp(-pow((edge - soft) / (soft + 1.5), 2.0)) * lit;
    c += f.tint.rgb * graze * 0.38 * s * (0.4 + 0.6 * near);
    // beams: lit air leaving the frame's edge toward the screen, streaked along the light
    let along = vec2<f32>(-dir.y, dir.x);
    let streak = vnoise(vec2<f32>(dot(p, along) / 5.0, f.time * 0.15)) * vnoise(vec2<f32>(dot(p, along) / 13.0 + 7.0, f.time * 0.08));
    let beam = pow(streak, 1.4) * lit * smoothstep(soft * 5.0 + 16.0, 0.0, edge - soft) * (0.35 + 0.65 * near);
    c += f.tint.rgb * beam * 0.55 * s;
    // dust drifting in the beams
    let drift = vec2<f32>(f.time * 2.2, f.time * -1.3);
    let cell = floor((p + drift) / 7.0);
    let at = cell * 7.0 + vec2<f32>(hash(cell), hash(cell + 2.3)) * 7.0 - drift;
    let mote = smoothstep(1.0, 0.0, distance(p, at)) * step(0.9, hash(cell + 5.1));
    c += vec3<f32>(1.0, 0.95, 0.85) * mote * (beam * 2.5 + graze * 0.8) * s;
    put(id.xy, c);
}
