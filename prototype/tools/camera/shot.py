"""One photo of the deck from the USB camera, exposure set by hand (no window): shot.py <out> <exposure> [focus]."""
import sys, time, cv2
out, exp = sys.argv[1], float(sys.argv[2])
cap = cv2.VideoCapture(0, cv2.CAP_DSHOW)
cap.set(cv2.CAP_PROP_FOURCC, cv2.VideoWriter_fourcc(*"MJPG"))
cap.set(cv2.CAP_PROP_FRAME_WIDTH, 1920)
cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 1080)
cap.set(cv2.CAP_PROP_AUTO_EXPOSURE, 0.25)  # manual on DirectShow
cap.set(cv2.CAP_PROP_EXPOSURE, exp)
if len(sys.argv) > 3:
    cap.set(cv2.CAP_PROP_AUTOFOCUS, 0)
    cap.set(cv2.CAP_PROP_FOCUS, float(sys.argv[3]))
t = time.time()
frame = None
while time.time() - t < 2.0:
    ok, f = cap.read()
    if ok:
        frame = f
print("exposure now", cap.get(cv2.CAP_PROP_EXPOSURE), "auto", cap.get(cv2.CAP_PROP_AUTO_EXPOSURE), "focus", cap.get(cv2.CAP_PROP_FOCUS), "size", None if frame is None else frame.shape)
cap.release()
cv2.imwrite(out, frame)
