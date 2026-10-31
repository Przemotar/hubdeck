// The upright copy of `matrix.wgsl` (owner, 2026-10-29: the deck may stand upright): drawn in the
// seen part's own coordinates (`to_view`), so the rain falls down the deck in every turn; landscape
// it is the original.
// Matrix, for developers: green code rain behind the icons, one screen through the fifteen
// windows. Columns of glyphs (made of strokes on a 3x4 grid, like the film's mirrored katakana and
// digits) fall at their own speeds, each column's head bright white-green, its trail fading, the
// glyphs in it changing now and then; a dim, slower rain further back. A press: the key's icon
// breaks into glowing glyphs, scatters and comes back together, and a cascade of bright glyphs
// pours from the key down over the keys below (`aux[0..3]`: x, y, start, 1 when on).
const CW = 9.0;
const CH = 11.0;
fn sd_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}
// a glyph `g` (0..1) in a 6x9 box: strokes chosen from the grid's 20 by the glyph's bits; the
// distance (px) from q, the box's top left at 0
fn glyph(q: vec2<f32>, g: f32) -> f32 {
    var d = 1e3;
    var n = 0;
    for (var i = 0; i < 20; i++) {
        let fi = f32(i);
        if (hash(vec2<f32>(g * 97.0, fi)) > 0.36 && !(i == 3 && n < 2)) { continue; }
        n += 1;
        var a = vec2<f32>(0.0);
        var b = vec2<f32>(0.0);
        if (i < 8) {
            // across: rows 0..3, left or right half
            let r = f32(i / 2);
            let h = f32(i % 2);
            a = vec2<f32>(h * 3.0, r * 3.0);
            b = a + vec2<f32>(3.0, 0.0);
        } else if (i < 17) {
            // down: columns 0..2, rows 0..2
            let j = i - 8;
            a = vec2<f32>(f32(j % 3) * 3.0, f32(j / 3) * 3.0);
            b = a + vec2<f32>(0.0, 3.0);
        } else {
            // a few slants
            let j = f32(i - 17);
            a = vec2<f32>(j * 3.0 - 3.0 * step(1.5, j), 3.0 + 3.0 * step(1.5, j));
            b = a + vec2<f32>(3.0, 3.0 * select(1.0, -1.0, j > 0.5 && j < 1.5));
        }
        d = min(d, sd_seg(q, a, b));
    }
    return d;
}
// one rain: columns of width `cw`, cells of `ch`; .x the glyph's light, .y 1 at a head
fn rain(p: vec2<f32>, cw: f32, ch: f32, speed: f32, trail: f32, seed: f32) -> vec2<f32> {
    let col = floor(p.x / cw);
    let row = floor(p.y / ch);
    let h1 = hash(vec2<f32>(col, seed));
    let v = (34.0 + 70.0 * h1) * speed;
    let span = view_size().y + trail * ch + 60.0;
    let head = fract(f.time * v / span + hash(vec2<f32>(seed, col))) * span - 30.0;
    let behind = (head - (row + 0.5) * ch) / ch;
    if (behind < -0.5 || behind > trail) { return vec2<f32>(0.0); }
    // the glyph in this cell, changing now and then (faster near the head)
    let rate = 1.5 + 5.0 * hash(vec2<f32>(col, row + seed));
    let g = hash(vec2<f32>(col * 1.7 + row, floor(f.time * rate + hash(vec2<f32>(row, col)) * 9.0) + seed));
    let q = p - vec2<f32>(col * cw + (cw - 6.0) * 0.5, row * ch + (ch - 9.0) * 0.5);
    let d = glyph(q, g);
    let ink = smoothstep(0.95, 0.35, d);
    let glow = exp(-max(d, 0.0) / 1.4) * 0.3;
    let fade = exp(-max(behind, 0.0) / (trail * 0.38));
    return vec2<f32>((ink + glow) * fade, select(0.0, 1.0, behind < 1.0));
}
@compute @workgroup_size(16, 16)
fn under(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let p = to_view(vec2<f32>(f32(id.x), f32(id.y)) + 0.5);
    var c = px(i32(id.x), i32(id.y)).rgb;
    let sp = f.speed;
    var col = vec3<f32>(0.0, 0.035, 0.02);
    // the far rain: small, dim, slow
    let back = rain(p + vec2<f32>(3.0, 0.0), 6.5, 8.0, 0.55 * sp, 22.0, 7.0);
    col += vec3<f32>(0.0, 0.32, 0.12) * back.x * 0.55;
    // the near rain
    let near = rain(p, CW, CH, sp, 16.0, 1.0);
    let green = mix(vec3<f32>(0.08, 0.85, 0.32), vec3<f32>(0.78, 1.0, 0.84), near.y);
    col += green * near.x;
    // a faint scanline and the screen's glow
    col *= 0.92 + 0.08 * step(0.5, fract(p.y * 0.5));
    col = clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, col, f.strength / 0.6 * 0.97 * (1.0 - keep_bg(c)));
    put(id.xy, c);
}
@compute @workgroup_size(16, 16)
fn over(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= f.size.x || id.y >= f.size.y) { return; }
    let pp = vec2<f32>(f32(id.x), f32(id.y)) + 0.5;
    let p = to_view(pp);
    var c = px(i32(id.x), i32(id.y)).rgb;
    for (var i = 0; i < 3; i++) {
        let b = f.aux[i];
        if (b.w < 0.5) { continue; }
        let age = f.time - b.z;
        if (age < 0.0 || age > 2.2) { continue; }
        let k = key_at(b.xy);
        if (k < 0) { continue; }
        let r = view_rect(keys[k]);
        // the icon breaks into glyphs: each 4 px block of it turns to glowing code for a while,
        // drifts down and comes back, the blocks one after another
        if (key_inside(pp, keys[k]) > 0.0 && age < 1.3) {
            let blk = floor((p - r.xy) / 4.0);
            let delay = hash(blk + vec2<f32>(f32(k), 3.0)) * 0.35;
            let t = clamp((age - delay) / 0.75, 0.0, 1.0);
            let out = sin(t * 3.14159);
            // where this block's pixel came from: it fell by `out` and comes back
            let fall = out * (6.0 + 10.0 * hash(blk + 1.7));
            let src = from_view(p - vec2<f32>(0.0, fall));
            let here = cover(i32(src.x), i32(src.y));
            let orig = px(i32(src.x), i32(src.y)).rgb;
            if (out > 0.02) {
                let g = hash(blk + floor(f.time * 12.0));
                let q = fract((p - r.xy) / vec2<f32>(4.0, 4.0)) * vec2<f32>(6.0, 9.0);
                let ink = smoothstep(1.6, 0.4, glyph(q, g));
                let code = vec3<f32>(0.2, 1.0, 0.45) * (0.45 + 0.8 * ink);
                let shown = mix(orig, code, smoothstep(0.1, 0.6, out));
                // the background where the block left, the block where it went
                let bg = px(i32(id.x), i32(id.y)).rgb * (1.0 - cover(i32(id.x), i32(id.y)) * out);
                c = mix(bg, shown, here);
            }
        }
        // the cascade: bright code pouring from the key down over the keys below
        let cx = r.x + r.z * 0.5;
        if (abs(p.x - cx) < r.z * 0.45 && p.y > r.y + r.w * 0.5) {
            let colx = floor((p.x - cx) / CW);
            let v = 210.0 + 90.0 * hash(vec2<f32>(colx, f32(k)));
            let headp = r.y + r.w * 0.5 + (age - 0.15 * hash(vec2<f32>(f32(k), colx))) * v;
            let behind = (headp - p.y) / CH;
            if (behind > -0.5 && behind < 9.0) {
                let row = floor(p.y / CH);
                let g = hash(vec2<f32>(colx * 3.1 + row, floor(f.time * 10.0)));
                let q = p - vec2<f32>(cx + colx * CW + (CW - 6.0) * 0.5, row * CH + (CH - 9.0) * 0.5);
                let ink = smoothstep(0.95, 0.35, glyph(q, g)) + exp(-max(glyph(q, g), 0.0) / 1.3) * 0.35;
                let fade = exp(-max(behind, 0.0) / 3.0) * (1.0 - smoothstep(1.4, 2.2, age)) * smoothstep(-0.5, 0.2, behind);
                c += mix(vec3<f32>(0.1, 0.9, 0.35), vec3<f32>(0.8, 1.0, 0.85), select(0.0, 1.0, behind < 1.0)) * ink * fade * 0.9;
            }
        }
    }
    put(id.xy, c);
}
