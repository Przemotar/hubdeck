"""Checks the living deck's WGSL shaders the way the app compiles them (the prelude from gpu.rs, then
each layer's body), with the GPU's own validator, and prints each error in full. The app's log only
says "does not compile: Validation Error" (session 33: eyes never ran on the owner's deck because
`target` is a reserved word in WGSL).

    <python with the `wgpu` package> scripts/wgsl-check.py [shader.wgsl ...]   # default: all

No window opens; needs a graphics card."""
import glob, os, re, sys
import wgpu

# the validator's box drawing would crash print() on a cp1250 console (python -I ignores
# PYTHONIOENCODING)
sys.stdout.reconfigure(errors="replace")

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
eng = os.path.join(root, "app", "src-tauri", "src", "engine")
gpu_rs = open(os.path.join(eng, "gpu.rs"), encoding="utf-8").read()
prelude = re.search(r'const PRELUDE: &str = r#"(.*?)"#;', gpu_rs, re.S).group(1)
shaders_rs = open(os.path.join(eng, "shaders.rs"), encoding="utf-8").read()
bodies = {m.group(1).lower(): m.group(2) for m in re.finditer(r'pub const (\w+): &str = r#"(.*?)"#;', shaders_rs, re.S)}
files = sys.argv[1:] or sorted(glob.glob(os.path.join(eng, "shaders", "*.wgsl")))
for f in files:
    bodies[os.path.splitext(os.path.basename(f))[0]] = open(f, encoding="utf-8").read()
device = wgpu.utils.get_default_device()
bad = 0
for name, body in bodies.items():
    if sys.argv[1:] and not any(os.path.basename(a).startswith(name) for a in sys.argv[1:]):
        continue
    try:
        device.create_shader_module(code=prelude + "\n" + body)
        print(f"ok    {name}")
    except Exception as e:
        bad += 1
        print(f"ERROR {name}: {e}")
sys.exit(1 if bad else 0)
