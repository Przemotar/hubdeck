"""A short film of the deck from the USB camera at 30 frames a second (ffmpeg, MJPG; the manual
exposure set first through OpenCV stays on the camera), with presses sent through HubDeck's API
at given seconds: film.py <out.mp4> <seconds> [key@second ...]. No window opens."""
import json, os, subprocess, sys, time, urllib.request, cv2
out, seconds = sys.argv[1], float(sys.argv[2])
presses = sorted((float(p.split("@")[1]), int(p.split("@")[0])) for p in sys.argv[3:])
d = os.path.join(os.environ["APPDATA"], "dev.hubdeck.app")
port = open(os.path.join(d, "api-http-port")).read().strip()
token = open(os.path.join(d, "api-token")).read().strip()
def press(key):
    req = urllib.request.Request(f"http://127.0.0.1:{port}/api/effects/press", method="POST", data=json.dumps({"key": key}).encode(), headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"})
    try:
        urllib.request.urlopen(req, timeout=3).read()
    except Exception as e:
        print("press failed", e)
cap = cv2.VideoCapture(0, cv2.CAP_DSHOW)
cap.set(cv2.CAP_PROP_AUTO_EXPOSURE, 0.25); cap.set(cv2.CAP_PROP_EXPOSURE, float(os.environ.get("EXPOSURE", "-8")))
cap.read(); cap.release(); time.sleep(0.5)
ff = subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-rtbufsize", "300M", "-f", "dshow", "-vcodec", "mjpeg", "-video_size", "1920x1080", "-framerate", "30",
                       "-i", "video=USB Camera", "-t", f"{seconds + 0.8:.1f}", "-vf", "crop=1250:780:330:80,scale=1000:-2", "-c:v", "libx264", "-preset", "veryfast", "-crf", "20",
                       "-pix_fmt", "yuv420p", "-movflags", "+faststart", out])
start = time.time() + 0.8
for at, key in presses:
    time.sleep(max(0.0, start + at - time.time()))
    press(key)
ff.wait()
print("film", out, os.path.getsize(out) // 1024, "KB")
