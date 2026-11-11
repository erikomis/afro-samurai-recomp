// Controls page: the bindings in input.toml (PS3_INPUT_MAP, read by the
// game's cellPad; format in the toolkit's libs/input/pad_input_map.h, the
// same file the inFAMOUS 2 and Resistance ports' launchers write). Click a
// cell, then press the key, mouse button or controller button to bind it; the
// rows light up while their inputs are held, and the sticks below follow the
// controller, which together are the controller test. The layout shown is
// always the full one: the port's own (port/input.toml, the keyboard as a
// pad; lib.rs sends it as `defaults`) with the player's changes on top, and
// that is what a change writes to the player's file. Until then there is no
// such file and Play passes the port's.
import { invoke } from "@tauri-apps/api/core";
import { Key, t } from "./i18n";
import { flashSaved } from "./ui";

export type Device = "keyboard" | "mouseButtons" | "controller";
export type Bindings = Record<Device, Record<string, string[]>>;
interface InputMapView {
  path: string;
  exists: boolean;
  /** The layout in effect, every control listed. */
  bindings: Bindings;
  /** The port's own layout, also in full. */
  defaults: Bindings;
}

const DEVICES: Device[] = ["keyboard", "mouseButtons", "controller"];

/** PS3 controls, in the screen's order, with the label shown for each. */
const CONTROLS: { id: string; label: string | Key }[] = [
  { id: "cross", label: "✕" },
  { id: "circle", label: "○" },
  { id: "square", label: "□" },
  { id: "triangle", label: "△" },
  { id: "l1", label: "L1" },
  { id: "r1", label: "R1" },
  { id: "l2", label: "L2" },
  { id: "r2", label: "R2" },
  { id: "l3", label: "L3" },
  { id: "r3", label: "R3" },
  { id: "start", label: "START" },
  { id: "select", label: "SELECT" },
  { id: "up", label: "controls.dpad" },
  { id: "down", label: "controls.dpad" },
  { id: "left", label: "controls.dpad" },
  { id: "right", label: "controls.dpad" },
  { id: "ls_up", label: "controls.ls" },
  { id: "ls_down", label: "controls.ls" },
  { id: "ls_left", label: "controls.ls" },
  { id: "ls_right", label: "controls.ls" },
  { id: "rs_up", label: "controls.rs" },
  { id: "rs_down", label: "controls.rs" },
  { id: "rs_left", label: "controls.rs" },
  { id: "rs_right", label: "controls.rs" },
];
const ARROW: Record<string, string> = { up: "↑", down: "↓", left: "←", right: "→" };

const noBindings = (): Bindings => ({ keyboard: {}, mouseButtons: {}, controller: {} });
/** The port's own layout (input_map.rs, port_defaults), as get_input_map sent it. */
let defaults: Bindings = noBindings();

// ---- input names --------------------------------------------------------------
/** KeyboardEvent.code -> the game's key name, for the keys with a name of their own. */
const NAMED_KEYS: Record<string, string> = {
  Space: "Space", Enter: "Return", Tab: "Tab", Backspace: "Backspace", Delete: "ForwardDelete",
  ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
  Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown", Insert: "Help", Help: "Help",
  Minus: "Minus", Equal: "Equal", BracketLeft: "LeftBracket", BracketRight: "RightBracket",
  Backslash: "Backslash", Semicolon: "Semicolon", Quote: "Quote", Comma: "Comma", Period: "Period",
  Slash: "Slash", Backquote: "Grave", IntlBackslash: "Section",
  NumpadEnter: "KeypadEnter", NumpadAdd: "KeypadPlus", NumpadSubtract: "KeypadMinus",
  NumpadMultiply: "KeypadMultiply", NumpadDivide: "KeypadDivide", NumpadDecimal: "KeypadDecimal",
  NumpadEqual: "KeypadEquals", NumLock: "KeypadClear",
  ShiftLeft: "Shift", ShiftRight: "RightShift", ControlLeft: "Control", ControlRight: "RightControl",
  AltLeft: "Option", AltRight: "RightOption",
};
/** KeyboardEvent.code -> the game's key name (macOS key positions). */
function keyName(code: string): string | null {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1];
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1];
  m = /^F([0-9]{1,2})$/.exec(code);
  if (m && Number(m[1]) >= 1 && Number(m[1]) <= 20) return code;
  m = /^Numpad([0-9])$/.exec(code);
  if (m) return `Keypad${m[1]}`;
  return NAMED_KEYS[code] ?? null; // Cmd, Caps Lock, Fn, Esc: the game's own or not keys
}

/** MouseEvent.button -> the game's mouse button name. */
const MOUSE_BUTTONS = ["Left", "Middle", "Right", "Button4", "Button5"];
/** Gamepad standard mapping index -> SDL GameController name. */
const PAD_INDEX = [
  "a", "b", "x", "y", "leftshoulder", "rightshoulder", "lefttrigger", "righttrigger",
  "back", "start", "leftstick", "rightstick", "dpup", "dpdown", "dpleft", "dpright", "guide", "touchpad",
];
/** SDL inputs offered in the list, with what they are on a PlayStation / Xbox pad. */
const PAD_INPUTS: [string, string][] = [
  ["a", "✕ / A"], ["b", "○ / B"], ["x", "□ / X"], ["y", "△ / Y"],
  ["leftshoulder", "L1 / LB"], ["rightshoulder", "R1 / RB"],
  ["lefttrigger", "L2 / LT"], ["righttrigger", "R2 / RT"],
  ["leftstick", "L3 / LS"], ["rightstick", "R3 / RS"],
  ["back", "Create, Share / View"], ["start", "Options / Menu"], ["guide", "PS / Xbox"],
  ["touchpad", "Touchpad"], ["misc1", "Mic / Share (Xbox)"],
  ["dpup", "D-pad ↑"], ["dpdown", "D-pad ↓"], ["dpleft", "D-pad ←"], ["dpright", "D-pad →"],
  ["paddle1", "Paddle 1"], ["paddle2", "Paddle 2"], ["paddle3", "Paddle 3"], ["paddle4", "Paddle 4"],
];
const PAD_LABEL = new Map(PAD_INPUTS);

/** Canonical spelling of a name read from the file (the game ignores case). */
const CANON = new Map<string, string>();
for (const n of Object.values(NAMED_KEYS)) CANON.set(n.toLowerCase(), n);
for (const n of [...MOUSE_BUTTONS, "Button6", "Button7", "Button8", "WheelUp", "WheelDown", "Escape"]) CANON.set(n.toLowerCase(), n);
for (const [n] of PAD_INPUTS) CANON.set(n, n);
/** The pad's "a b x y" are also letter keys: a controller name only in the controller column. */
const PAD_NAMES = new Set(PAD_INPUTS.map(([n]) => n));
const canon = (n: string, dev: Device) => {
  const low = n.toLowerCase();
  if (dev === "controller") return PAD_NAMES.has(low) ? low : n;
  if (dev === "keyboard" && /^([a-z]|f[0-9]{1,2})$/i.test(n)) return n.toUpperCase();
  const hit = CANON.get(low);
  if (hit && !PAD_NAMES.has(hit)) return hit;
  return /^keypad[0-9]$/i.test(n) ? `Keypad${n.slice(-1)}` : n;
};

function inputLabel(dev: Device, name: string): string {
  if (dev === "controller") return PAD_LABEL.get(name) ?? name;
  if (dev === "mouseButtons") {
    const k = `controls.mouse.${name}` as Key;
    const v = t(k);
    return v === k ? name : v;
  }
  return ({ Up: "↑", Down: "↓", Left: "←", Right: "→", Escape: "Esc" } as Record<string, string>)[name] ?? name;
}

// ---- state --------------------------------------------------------------------
const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
let bindings: Bindings = noBindings();
let mapPath = "";
let mapExists = false;
let capture: { dev: Device; ctl: string; padBase: boolean[] } | null = null;
const heldKeys = new Set<string>();
const heldMouse = new Set<string>();
let heldPad = new Set<string>();
let padFrame = 0;

function clone(b: Bindings): Bindings {
  return {
    keyboard: Object.fromEntries(Object.entries(b.keyboard).map(([k, v]) => [k, [...v]])),
    mouseButtons: Object.fromEntries(Object.entries(b.mouseButtons).map(([k, v]) => [k, [...v]])),
    controller: Object.fromEntries(Object.entries(b.controller).map(([k, v]) => [k, [...v]])),
  };
}

/** The port's layout with `file`'s entries on top: the full table the screen
 * shows and saves. */
function merged(file: Bindings): Bindings {
  const b = clone(defaults);
  for (const d of DEVICES) {
    for (const c of CONTROLS) b[d][c.id] ??= [];
    for (const [k, v] of Object.entries(file[d] ?? {})) b[d][k] = v.map((n) => canon(n, d));
  }
  return b;
}

/** What get_input_map and reset_input_map answer. */
function load(v: InputMapView) {
  defaults = clone({ ...noBindings(), ...v.defaults });
  bindings = merged({ ...noBindings(), ...v.bindings });
  mapPath = v.path;
  mapExists = v.exists;
}

async function save() {
  try {
    mapPath = await invoke<string>("save_input_map", { bindings });
    mapExists = true;
    renderPath();
    flashSaved();
  } catch (e) {
    note(String(e));
  }
}

function note(msg: string) {
  $("controls-msg").textContent = msg;
}

/** The d-pad direction and the left-stick direction that go together: the
 * port's layout puts one key on both (the arrows), so binding an input to
 * one leaves it on the other. */
function twin(ctl: string): string {
  if (ctl.startsWith("ls_")) return ctl.slice(3);
  return ARROW[ctl] ? `ls_${ctl}` : "";
}

/** Bind `name` to one control, taking it away from the others of that device. */
function bind(dev: Device, ctl: string, name: string) {
  const taken: string[] = [];
  for (const [k, v] of Object.entries(bindings[dev])) {
    if (k !== ctl && k !== twin(ctl) && v.includes(name)) {
      bindings[dev][k] = v.filter((n) => n !== name);
      taken.push(controlLabel(k));
    }
  }
  bindings[dev][ctl] = [name];
  note(taken.length ? t("controls.moved", { input: inputLabel(dev, name), from: taken.join(", ") }) : "");
  endCapture();
  save();
}

function controlLabel(id: string): string {
  const c = CONTROLS.find((x) => x.id === id);
  if (!c) return id;
  const dir = id.replace(/^(ls_|rs_)/, "");
  return c.label.startsWith("controls.") ? `${t(c.label as Key)} ${ARROW[dir] ?? ""}`.trim() : c.label;
}

/** The file's path, or that there is none yet (the port's layout). */
function renderPath() {
  $("controls-path").textContent = mapPath;
  $("controls-default").classList.toggle("hidden", mapExists);
}

// ---- the table ------------------------------------------------------------------
export function renderControls() {
  const body = $("controls-body");
  body.innerHTML = "";
  for (const c of CONTROLS) {
    const tr = document.createElement("tr");
    tr.dataset.ctl = c.id;
    const th = document.createElement("th");
    th.textContent = controlLabel(c.id);
    tr.appendChild(th);
    for (const d of DEVICES) {
      const td = document.createElement("td");
      const cell = document.createElement("button");
      cell.className = "bind";
      const capturing = capture?.dev === d && capture.ctl === c.id;
      cell.classList.toggle("capturing", capturing);
      const names = bindings[d][c.id] ?? [];
      cell.textContent = capturing
        ? t(d === "keyboard" ? "controls.pressKey" : d === "mouseButtons" ? "controls.pressMouse" : "controls.pressPad")
        : names.length ? names.map((n) => inputLabel(d, n)).join(" / ") : "—";
      cell.title = names.join(", ");
      cell.addEventListener("click", (ev) => {
        ev.stopPropagation();
        if (capture?.dev === d && capture.ctl === c.id) return; // the capture takes this cell's own clicks
        startCapture(d, c.id);
      });
      td.appendChild(cell);
      if (capturing && d === "controller") {
        const sel = document.createElement("select");
        sel.className = "pad-pick";
        sel.append(new Option(t("controls.pickPad"), ""), ...PAD_INPUTS.map(([n, label]) => new Option(label, n)));
        sel.addEventListener("click", (ev) => ev.stopPropagation());
        sel.addEventListener("change", () => sel.value && bind("controller", c.id, sel.value));
        td.appendChild(sel);
      }
      if (names.length && !capturing) {
        const x = document.createElement("button");
        x.className = "unbind";
        x.textContent = "×";
        x.title = t("controls.unbind");
        x.addEventListener("click", (ev) => {
          ev.stopPropagation();
          bindings[d][c.id] = [];
          note("");
          renderControls();
          save();
        });
        td.appendChild(x);
      }
      tr.appendChild(td);
    }
    body.appendChild(tr);
  }
  renderHeld();
}

/** The rows whose inputs are held right now (the button test). */
function renderHeld() {
  document.querySelectorAll<HTMLTableRowElement>("#controls-body tr").forEach((tr) => {
    const id = tr.dataset.ctl!;
    const on =
      (bindings.keyboard[id] ?? []).some((n) => heldKeys.has(n)) ||
      (bindings.mouseButtons[id] ?? []).some((n) => heldMouse.has(n)) ||
      (bindings.controller[id] ?? []).some((n) => heldPad.has(n));
    tr.classList.toggle("held", on);
  });
}

function startCapture(dev: Device, ctl: string) {
  capture = { dev, ctl, padBase: padButtons() };
  note(t("controls.captureHint"));
  renderControls();
}

function endCapture() {
  capture = null;
  renderControls();
}

const visible = () => $("view-controls").classList.contains("active");

// ---- input events -------------------------------------------------------------------
window.addEventListener("keydown", (ev) => {
  if (!visible()) return;
  if (ev.metaKey) return; // Cmd shortcuts stay the app's
  const name = keyName(ev.code);
  if (capture) {
    ev.preventDefault();
    if (ev.code === "Escape") {
      note("");
      endCapture();
      return;
    }
    if (capture.dev !== "keyboard") return;
    if (!name) {
      note(t("controls.notBindable", { key: ev.code }));
      return;
    }
    bind("keyboard", capture.ctl, name);
    return;
  }
  if (name && !(ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement || ev.target instanceof HTMLTextAreaElement)) {
    // a bound key is the test's alone (Tab, Return, Space and the arrows would move the page)
    if (Object.values(bindings.keyboard).some((v) => v.includes(name))) ev.preventDefault();
    heldKeys.add(name);
    renderHeld();
  }
}, true);
window.addEventListener("keyup", (ev) => {
  const name = keyName(ev.code);
  if (name && heldKeys.delete(name)) renderHeld();
}, true);
window.addEventListener("blur", () => {
  heldKeys.clear();
  heldMouse.clear();
  renderHeld();
});

function onMouseDown(ev: MouseEvent) {
  if (!visible()) return;
  const name = MOUSE_BUTTONS[ev.button];
  if (capture?.dev === "mouseButtons" && name) {
    ev.preventDefault();
    ev.stopPropagation();
    bind("mouseButtons", capture.ctl, name);
    return;
  }
  if (name && (ev.target as HTMLElement).closest("#controls-test")) {
    heldMouse.add(name);
    renderHeld();
  }
}
window.addEventListener("mousedown", onMouseDown, true);
window.addEventListener("mouseup", (ev) => {
  const name = MOUSE_BUTTONS[ev.button];
  if (name && heldMouse.delete(name)) renderHeld();
}, true);
window.addEventListener("contextmenu", (ev) => {
  if (visible() && (capture?.dev === "mouseButtons" || (ev.target as HTMLElement).closest("#controls-test"))) ev.preventDefault();
}, true);
window.addEventListener("wheel", (ev) => {
  if (!visible() || capture?.dev !== "mouseButtons" || ev.deltaY === 0) return;
  ev.preventDefault();
  bind("mouseButtons", capture.ctl, ev.deltaY < 0 ? "WheelUp" : "WheelDown");
}, { passive: false, capture: true });
// a click anywhere else ends a keyboard or controller capture
window.addEventListener("click", (ev) => {
  if (capture && capture.dev !== "mouseButtons" && !(ev.target as HTMLElement).closest(".pad-pick")) {
    note("");
    endCapture();
  }
});

/** The first connected pad, if the web view has the Gamepad API. */
function firstPad(): Gamepad | null {
  const pads = typeof navigator.getGamepads === "function" ? navigator.getGamepads() : [];
  for (const p of pads) if (p && p.connected) return p;
  return null;
}

/** The first connected standard-mapping pad's buttons. */
function padButtons(): boolean[] {
  return firstPad()?.buttons.map((b) => b.pressed || b.value > 0.5) ?? [];
}

/** The controller test: its name and where its sticks are. */
function renderPad(pad: Gamepad | null) {
  $("pad-name").textContent = pad
    ? t("controls.connected", { name: pad.id })
    : typeof navigator.getGamepads === "function" ? t("controls.noPad") : t("controls.noApi");
  const stick = (id: string, x = 0, y = 0) => {
    const dot = $(id).querySelector("i") as HTMLElement;
    dot.style.left = `${50 + x * 42}%`;
    dot.style.top = `${50 + y * 42}%`;
  };
  stick("stick-l", pad?.axes[0], pad?.axes[1]);
  stick("stick-r", pad?.axes[2], pad?.axes[3]);
}

function pollPad() {
  padFrame = 0;
  if (!visible()) return;
  renderPad(firstPad());
  const now = padButtons();
  const held = new Set<string>();
  now.forEach((down, i) => down && PAD_INDEX[i] && held.add(PAD_INDEX[i]));
  if (capture?.dev === "controller") {
    const fresh = now.findIndex((down, i) => down && !capture!.padBase[i] && PAD_INDEX[i]);
    if (fresh >= 0) {
      bind("controller", capture.ctl, PAD_INDEX[fresh]);
    } else {
      capture.padBase = capture.padBase.map((b, i) => b && !!now[i]); // released buttons can count next
    }
  }
  if ([...held].join() !== [...heldPad].join()) {
    heldPad = held;
    renderHeld();
  }
  padFrame = requestAnimationFrame(pollPad);
}

// ---- entry points -------------------------------------------------------------------
/** Called when the Controls view opens or closes. */
export async function showControls(on = true) {
  if (!on) {
    if (capture) endCapture();
    return;
  }
  try {
    load(await invoke<InputMapView>("get_input_map"));
  } catch (e) {
    note(String(e));
  }
  renderPath();
  renderControls();
  if (!padFrame) padFrame = requestAnimationFrame(pollPad);
}

export function initControls() {
  $("controls-reset").addEventListener("click", async () => {
    // the player's file goes (kept as input.toml.bak): Play passes the port's
    // own again (with the mouse camera on, a new file with the port's layout)
    try {
      load(await invoke<InputMapView>("reset_input_map"));
      flashSaved();
    } catch (e) {
      note(String(e));
      return;
    }
    endCapture();
    note(t("controls.resetDone"));
    renderPath();
  });
  // without a file yet this shows the folder it will be written to
  $("controls-open").addEventListener("click", () => mapPath && invoke("reveal", { path: mapPath }));
  window.addEventListener("gamepadconnected", () => {
    if (visible() && !padFrame) padFrame = requestAnimationFrame(pollPad);
  });
}
