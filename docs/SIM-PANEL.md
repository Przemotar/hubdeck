# Sim panel: the deck as a joystick

HubDeck can make the deck look like a game controller to flight, racing and space sims: keys
become joystick buttons, faders and knobs become axes (throttle, mixture, trims), rollers tap
buttons. The sim does not know HubDeck; you bind the controls in the sim's own control settings,
the same way as for any joystick.

It works through **vJoy**, a free virtual joystick driver. HubDeck does not ship it: you install
it yourself, once. **Windows only** (vJoy has no macOS version; on a Mac the joystick actions say
so).

## 1. Install vJoy

1. Download the installer of the signed fork for Windows 10 and 11:
   <https://github.com/BrunnerInnovation/vJoy/releases> (version 2.2.2.0 at the time of writing).
2. Run it and accept the driver. Restart Windows if it asks.
3. If the installer fails on Windows 11, see the fork's issues page; a leftover older vJoy (uninstall
   it first) or Windows "Memory integrity" are the usual causes.

HubDeck finds vJoy by itself (the installer records its folder; otherwise
`C:\Program Files\vJoy\x64`). Nothing to set in HubDeck.

## 2. Set up the virtual joystick

Open **Configure vJoy** from the Start menu:

1. Pick device **1** (tab at the top), tick **Enable vJoy** at the bottom.
2. **Axes**: tick all eight (X, Y, Z, Rx, Ry, Rz, Slider, Dial/Slider2).
3. **Number of buttons**: **32** (the Sim panel uses 19; more costs nothing).
4. **POV hat switch**: 1, **Continuous** if you want diagonals on the "Joystick hat" action.
5. **Apply**.

Check it: **Monitor vJoy** (Start menu) shows the axes and buttons live; press a deck key with the
Sim panel and its button lights up. Windows' own "Set up USB game controllers" (`joy.cpl`) >
vJoy Device > Properties shows the same.

If another program (Joystick Gremlin, FreePIE...) already feeds device 1, give HubDeck device 2:
enable it in Configure vJoy and set "vJoy device" to 2 on the actions.

## 3. Add the Sim panel to HubDeck

"+" > **Sim panel**. It switches in by itself when one of these is in front: Microsoft Flight
Simulator 2020 and 2024, DCS World, X-Plane, iRacing, Star Citizen (edit the list in the profile).

| Control | Does | vJoy |
|---|---|---|
| Keys 1-15 | Buttons B1-B15, held as long as the key (no delay for hold or double press) | buttons 1-15 |
| Fader 1 | Throttle | Z axis |
| Fader 2 | Mixture | Rx axis |
| Fader 3 | Propeller | Ry axis |
| Fader 4 | Flaps | Slider |
| Fader 5 | Trim | Dial (Slider2) |
| Dial | Fine trim (0.5 % per step) on the same axis as fader 5; press = center | Dial (Slider2) |
| Knobs 1-3 | Axes X, Y, Rz (2 % per step); press = center | X, Y, Rz |
| Roller 1 | Up taps B16, down taps B17 | buttons 16, 17 |
| Roller 2 | Up taps B18, down taps B19 | buttons 18, 19 |

Each control can be changed in the inspector:

- **Joystick button**: the button a key holds; on a knob, roller or the dial, the button tapped per
  step in each direction (each tap is held 40 ms so sims that read the joystick once a frame see
  it).
- **Joystick axis**: which axis; how far one step of a knob, roller or the dial moves it; what a
  press does (center, minimum, maximum, nothing). A knob continues from the value HubDeck set last,
  also one set by a fader on the same axis.
- **Joystick hat**: a direction held while the key is down (a D-pad / look around).

When the deck is paused from the tray, the computer sleeps or HubDeck quits, every button is let go
and the joystick is handed back.

## 4. Bind the controls in the sim

General rule: open the sim's control settings, pick the **vJoy Device**, choose the sim function,
click "assign" / "scan input", then move the fader or press the key. Many sims give a new joystick
a default layout (often X/Y as aileron/elevator): **clear the vJoy device's default bindings
first** so a knob does not fly the plane.

- **Microsoft Flight Simulator 2020 / 2024**: Options (Settings) > Controls > pick **vJoy Device**
  at the top > create a new profile (empty) > search for the function ("Throttle axis",
  "Mixture axis", "Propeller axis", "Flaps axis", "Elevator trim axis") > Search by input / Scan
  input > move the fader > Validate. Buttons the same way (e.g. "Landing gear (toggle)").
- **DCS World**: Options > Controls > pick the aircraft > the vJoy Device column. Clear its
  default axes first ("Clear category" for the column). Axes: "Axis Assign" > pick the function
  (e.g. "Thrust") > double-click the vJoy cell > move the fader; "Axis Tune" to invert or add a
  curve. Buttons: double-click the vJoy cell of a function and press the key.
- **iRacing**: Options > Controls. Buttons: click a function (e.g. "Pit limiter", "Brake bias +")
  and press the key. Axes such as handbrake or clutch go through the calibration wizard
  ("Calibrate") where you move the fader end to end.
- **Star Citizen**: Options > Keybindings > Advanced Controls Customization. The vJoy device is a
  joystick there (js1/js2; see Options > Devices for the order). Bind functions to its buttons
  and axes (e.g. throttle on Z, strafe on a knob).
- **X-Plane 12**: Settings > Joystick > pick vJoy Device > move a fader and choose what the axis
  does; buttons below.

## Troubleshooting

| HubDeck says | Do |
|---|---|
| "Install vJoy to use the deck as a joystick" | Install vJoy (step 1). HubDeck looks again 10 s later; no restart needed. |
| "vJoy is installed but switched off" | Configure vJoy > Enable vJoy. |
| "vJoy device 1 is not set up" | Configure vJoy: enable that device (step 2). |
| "... is used by another program" | Close the other feeder or use another device number. |
| "... has 8 buttons, button 16 needs more" | Configure vJoy: raise the number of buttons. |
| "... has no RZ axis" | Configure vJoy: tick the axis. |
| "... has four directions only" | Configure vJoy: POV hat switch Continuous, for diagonals. |

The sim does not see changes: some sims list joysticks only when they start; start the sim after
vJoy is configured.
