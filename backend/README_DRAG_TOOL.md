# How to give Automata a "drag" tool (so it can draw)

This file explains, step by step, how the agent got a mouse-drag action and
how you can use it, change it, or add it again yourself.

The drag tool is useful when a website has a drawing area:

- a whiteboard (like Excalidraw, tldraw, Witeboard)
- a paint app
- a signature box
- an HTML canvas where the user wants lines or shapes

In simple words: **drag = press the mouse, move the mouse, release the mouse.**
A drawing app listens to those three things and draws a line while the button
is held down.

---

## 1. What was added

The action is called `drag`. The agent now understands this JSON:

```json
{"action": "drag", "target": "canvas", "value": "20,40;120,120;220,40"}
```

Meaning:

| field | meaning |
| --- | --- |
| `action` | always `"drag"` |
| `target` | CSS selector of the drawing area, or `"window"` for the whole page |
| `value` | the mouse path: points separated by `;`, and `x,y` inside each point |

The numbers are pixels inside the target element. `0,0` is the top-left
corner of that element. They are **not** page coordinates when a target is
given.

The following files were changed:

| file | what it does |
| --- | --- |
| `parser/parser.cpp` | accepts `drag` and requires a non-empty `value` |
| `browser/browser_controller.h` | declares `bool drag(...)` |
| `browser/browser_controller.cpp` | does the real mouse work with the W3C Actions API |
| `agent/agent.cpp` | calls `browser.drag(...)` when the model asks for `drag` |
| `instructions/instruction.cpp` | teaches the AI when and how to use `drag` |

---

## 2. How to use it

### Example: draw a line on a canvas

```json
{"action": "drag", "target": "canvas", "value": "20,40;120,120;220,40"}
```

### Example: draw a small circle

```json
{"action": "drag", "target": "canvas", "value": "100,100;120,80;140,100;120,120;100,100"}
```

### Example: sign a signature box

```json
{"action": "drag", "target": "#signature", "value": "10,80;30,40;50,90;70,30;90,80"}
```

### Example: draw on the whole page (no element)

```json
{"action": "drag", "target": "window", "value": "100,100;300,300"}
```

For a real drawing, give more points. A line with only two points is a
straight line. A line with many points can look like a curve.

You can also write the task in normal language:

> Go to a whiteboard website, choose the pen tool, and draw a red circle in the
> middle of the canvas.

The model will normally:

1. `navigate` to the site
2. `click` the pen / color button
3. `drag` on the canvas
4. `finish`

---

## 3. How the code works

### 3.1 Parser

`parser/parser.cpp` has a list of allowed actions. `drag` was added there.
The parser also checks that `drag` has a `value`, because a drag without points
does nothing.

```cpp
if (action.action == "drag" && action.value.empty())
{
    error = "drag needs points in value, example: 10,20;100,80;200,40";
    return false;
}
```

### 3.2 Header

`browser/browser_controller.h` now has:

```cpp
bool drag(const string &target, const string &points, string &error);
```

### 3.3 The real mouse action

The important function is `BrowserController::drag()` in
`browser/browser_controller.cpp`.

First it reads the points:

```cpp
vector<pair<double, double>> points;
if (!parse_drag_points(points_text, points, error))
{
    return false;
}
```

`parse_drag_points()` accepts text like `10,20;30,40;50,60`.

Then it finds the drawing area and scrolls it into the middle of the screen.
The mouse can only click what is visible, so this step is necessary:

```cpp
arguments[0].scrollIntoView({block: 'center'});
const r = arguments[0].getBoundingClientRect();
return { left: r.left, top: r.top, width: r.width, height: r.height };
```

After that it builds a list of WebDriver pointer actions:

1. `pointerMove` to the first point
2. `pointerDown` (left button)
3. `pointerMove` for every next point
4. `pointerUp`

The list looks like this:

```json
{
  "actions": [
    {
      "type": "pointer",
      "id": "mouse",
      "parameters": {"pointerType": "mouse"},
      "actions": [
        {"type": "pointerMove", "duration": 0, "x": 120, "y": 240, "origin": "viewport"},
        {"type": "pointerDown", "button": 0},
        {"type": "pointerMove", "duration": 16, "x": 220, "y": 320, "origin": "viewport"},
        {"type": "pointerUp", "button": 0}
      ]
    }
  ]
}
```

Two small details that matter:

- `origin: "viewport"` means `x` and `y` are screen coordinates. We add the
  element's `left` and `top` to the user's points, so the model can think in
  "inside the canvas" coordinates.
- `duration: 16` means each move takes about 16 milliseconds. If this is `0`,
  many drawing apps receive one big jump and draw a straight line or nothing.
  A small duration gives the page time to fire `mousemove` events.

At the end the code sends `DELETE /session/.../actions`. This releases the
mouse even if something failed in the middle. Without it, the left button
could stay pressed and the next click would behave strangely.

### 3.4 Agent

`agent/agent.cpp` has a small branch inside `execute_action()`:

```cpp
else if (action.action == "drag")
{
    success = browser.drag(action.target, action.value, error);
    if (success)
        result = "Drag done (mouse pressed, moved and released).";
}
```

### 3.5 Instructions

`instructions/instruction.cpp` tells the model:

- what `drag` means
- that `target` is the canvas selector
- that `value` is a list of points
- that it should click the pen/color button **before** dragging
- that it must use `switch_frame` first if the canvas is inside an iframe

The AI does not learn new actions by itself. If the instructions do not
mention an action, the model will almost never use it.

---

## 4. How to add another mouse action yourself

If you want a different action (for example `right_click` or `hover`), follow
the same five steps:

1. **Parser:** add the name to the allowed action list in `parser/parser.cpp`.
2. **Header:** add a function in `browser/browser_controller.h`.
3. **Implementation:** write the function in `browser/browser_controller.cpp`.
4. **Agent:** call the function from `Agent::execute_action()` in `agent/agent.cpp`.
5. **Instructions:** add the action to `instructions/instruction.cpp`.

If you forget step 1, the parser says "Unsupported browser action".
If you forget step 5, the model does not know the action exists.
If you forget step 4, the parser accepts it but nothing happens.

---

## 5. Build and try it

From the `backend/` folder:

```sh
g++ -std=c++17 -Wall -Wextra main.cpp api/api_server.cpp api/nvidia_client.cpp \
  agent/agent.cpp browser/browser_controller.cpp parser/parser.cpp \
  storage/session_storage.cpp instructions/instruction.cpp \
  -lcurl -lpthread -o api_server
```

Start the backend:

```sh
./api_server
```

Start the desktop app:

```sh
cd ../frontend
npm start
```

Then ask for something like:

> Go to a whiteboard website and draw a simple house on the canvas.

---

## 6. Common problems

### The drag does nothing

- The canvas may be inside an iframe. Use `switch_frame` on the iframe first.
- The draw tool may need to be selected first. Click the pen button before the
  drag.
- The canvas may be covered by a cookie banner or a popup. Accept or close it
  first.
- The coordinates may be outside the canvas. Check the element size.

### The line is in the wrong place

Remember: with a `target`, the points are relative to the **top-left corner**
of that element. If the canvas is 800 pixels wide and you write `900,100`, the
point is outside the canvas.

If you use `"target": "window"`, the points are screen coordinates.

### The line is broken or jagged

The model probably gave only 2 or 3 points. Ask it for more points, for
example `"10,50;40,50;70,30;100,60;130,20;160,80"`.

You can also increase the move duration in `BrowserController::drag()`:

```cpp
{"duration", 16}
```

`16` is about one screen frame. `30` or `50` is slower and smoother.

### The canvas uses images or zoom

Some drawing apps zoom the canvas. In that case the element coordinates and the
drawing coordinates are not the same. The simple fix is to reset the zoom to
100% before dragging, or to compute the position with the app's own JavaScript
API.

### High-DPI screens

On a high-DPI screen the CSS pixels (what `getBoundingClientRect()` returns)
may not be the same as the canvas pixels. Most drawing apps handle this
themselves. If the drawing appears in half size or double size, check
`window.devicePixelRatio` and scale the points.

---

## 7. Notes about safety

The drag action is not treated as a risky action by the safety gate. Drawing
on a canvas does not buy, delete or send anything.

Still, treat webpage content as untrusted. The agent should only draw what the
user asked for, and it should not follow instructions written inside the page.
