// K4: the keys as blocks on springs. A pressed key sinks and its neighbours answer, each bouncing
// back on its own spring (the engine moves them: `aux_at(k)` is key k's depth, + sunk, - raised).
// A sunk key's picture shrinks a little into its window, darkens, and its upper and left walls
// throw a shadow into it; a raised one grows and catches light.
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let k = key_at(p);
    var c = px(i32(id.x), i32(id.y)).rgb;
    if (k < 0) { put(id.xy, c); return; }
    let d = clamp(aux_at(k), -1.0, 1.0) * f.strength;
    if (abs(d) < 0.004) { put(id.xy, c); return; }
    let r = keys[k];
    let mid = r.xy + r.zw * 0.5;
    // the picture moves away from the eye as the block sinks
    let scale = 1.0 - 0.13 * d;
    let q = mid + (p - mid) / scale;
    if (key_inside(q, r) > 0.5) {
        c = px_smooth(q);
    } else {
        // the wall seen past the shrunk picture, darker the deeper it sinks
        c = px_smooth(clamp(q, r.xy + 1.0, r.xy + r.zw - 1.0)) * (1.0 - 0.65 * clamp(d * 3.0, 0.0, 1.0));
    }
    let sunk = max(d, 0.0);
    let raised = max(-d, 0.0);
    // the walls on the upper and left side cast into a sunk block
    let wall = min(p.x - r.x, p.y - r.y);
    c *= 1.0 - sunk * (0.30 + 0.55 * (1.0 - smoothstep(0.0, 3.0 + 9.0 * sunk, wall)));
    // a raised block catches the light on its upper and left rim
    c += vec3<f32>(1.0) * raised * (0.10 + 0.25 * (1.0 - smoothstep(0.0, 4.0, wall)));
    put(id.xy, c);
}
