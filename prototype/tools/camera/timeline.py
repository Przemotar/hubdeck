"""Reads the frame counter of crates/mh-device/examples/counter.rs from a camera film, second by second: how many different pictures the key screen showed, and how many reads were unclear (a picture changing as the camera took it). The ten square centres are for the camera where it stood on 2026-10-26; take them from a calibration frame (all ten white) when it moves. Film: ffmpeg -f dshow -vcodec mjpeg -video_size 1920x1080 -framerate 30 -i "video=USB Camera" -t 75 -c:v copy test.avi. timeline.py <film>"""
pts=[(421,435),(459,434),(543,431),(587,429),(674,427),(719,426),(810,424),(858,423),(951,421),(999,420)]
cap=cv2.VideoCapture(sys.argv[1]); i=0; rows=[]
while True:
    ok,f=cap.read()
    if not ok: break
    g=cv2.cvtColor(f,cv2.COLOR_BGR2GRAY)
    vals=[int(g[y-4:y+5,x-4:x+5].mean()) for x,y in pts]
    bits=''.join('1' if v>110 else '0' if v<50 else '?' for v in vals)
    low=int(g[470:650,380:1040].mean())
    rows.append((i,bits,low)); i+=1
# per second: distinct clear numbers, unclear count, mean brightness of lower rows
for s in range(0,len(rows)//30):
    seg=rows[s*30:(s+1)*30]
    clear=[int(b,2) for _,b,_ in seg if '?' not in b]
    print(f"{s:3d}s distinct {len(set(clear)):3d} unclear {sum('?' in b for _,b,_ in seg):3d} low {sum(l for *_,l in seg)//30:3d} sample {seg[0][1]} {seg[15][1]}")
