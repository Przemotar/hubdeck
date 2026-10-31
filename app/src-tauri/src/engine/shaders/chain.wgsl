// A press that travels: chain lightning. An arc leaps from the pressed key to the next, then the
// next (three or four keys the engine picks, `aux[0]` = start, key 0, key 1, key 2; `aux[1]` = key
// 3, how many keys, 0, 1 when on); each bolt is a jagged white-violet line that flickers for a
// quarter of a second, and each key it strikes flashes. A flashing effect: the window warns of it.
fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let ab = b - a;
    let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
    return vec2<f32>(distance(p, a + ab * t), t);
}
fn centre(k: f32) -> vec2<f32> {
    let r = keys[i32(k)];
    return r.xy + r.zw * 0.5;
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    var c = px(i32(id.x), i32(id.y)).rgb;
    let a = f.aux[0];
    let b = f.aux[1];
    if (b.w < 0.5) { put(id.xy, c); return; }
    let ks = array<f32, 4>(a.y, a.z, a.w, b.x);
    let n = i32(b.y);
    let age = f.time - a.x;
    let col = mix(vec3<f32>(0.85, 0.80, 1.0), f.tint.rgb, 0.35);
    for (var i = 0; i < 3; i++) {
        if (i + 1 >= n) { break; }
        let t0 = f32(i) * 0.07;
        let life = age - t0;
        if (life < 0.0 || life > 0.32) { continue; }
        let start = centre(ks[i]);
        let end = centre(ks[i + 1]);
        // the bolt: a jagged line, its kinks changing every 40 ms
        let frame = floor(f.time * 25.0);
        let s = seg(p, start, end);
        let dir = normalize(end - start);
        let side = vec2<f32>(-dir.y, dir.x);
        let kink = (vnoise(vec2<f32>(s.y * 9.0, frame + f32(i) * 7.0)) - 0.5) * 18.0 * sin(s.y * 3.14159);
        let q = p - side * kink;
        let d = seg(q, start, end).x;
        let flick = step(0.25, hash(vec2<f32>(frame, f32(i))));
        let fade = 1.0 - life / 0.32;
        c += col * (smoothstep(1.4, 0.0, d) * 1.2 + exp(-d * d / 30.0) * 0.4) * flick * fade * f.strength;
        // the struck key flashes
        let rk = keys[i32(ks[i + 1])];
        if (key_inside(p, rk) > 0.0) {
            c += col * 0.45 * fade * flick * f.strength;
        }
    }
    put(id.xy, c);
}
