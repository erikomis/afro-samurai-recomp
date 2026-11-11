// Controls page: the port's built-in layout, which this runtime does not let
// a file remap. The keyboard drives pad 1 with fixed keys (the toolkit's
// libs/input/cellPad.c, pad_poll_keyboard: Z X A S, Q W, 1 2, Return, Tab,
// the arrows), and a controller goes through SDL2's GameController mapping
// (any PlayStation, Xbox or MFi pad, by position). The rows light up while
// their inputs are held, and the sticks below follow the controller, which
// together are the controller test. Settings > Entrada turns the keyboard off
// (PAD_NO_KEYBOARD=1).
import { Key, t } from "./i18n";

interface Row {
  label: string | Key;
  /** KeyboardEvent.code values that press it. */
  keys: string[];
  /** What the key caps say. */
  keyText: string;
  /** Gamepad standard-mapping button indices that press it. */
  pad: number[];
  padText: string;
}

/** cellPad.c's keyboard map, in its order, with the pad buttons SDL maps there. */
const ROWS: Row[] = [
  { label: "✕", keys: ["KeyZ"], keyText: "Z", pad: [0], padText: "✕ / A" },
  { label: "○", keys: ["KeyX"], keyText: "X", pad: [1], padText: "○ / B" },
  { label: "□", keys: ["KeyA"], keyText: "A", pad: [2], padText: "□ / X" },
  { label: "△", keys: ["KeyS"], keyText: "S", pad: [3], padText: "△ / Y" },
  { label: "L1", keys: ["KeyQ"], keyText: "Q", pad: [4], padText: "L1 / LB" },
  { label: "R1", keys: ["KeyW"], keyText: "W", pad: [5], padText: "R1 / RB" },
  { label: "L2", keys: ["Digit1"], keyText: "1", pad: [6], padText: "L2 / LT" },
  { label: "R2", keys: ["Digit2"], keyText: "2", pad: [7], padText: "R2 / RT" },
  { label: "L3", keys: [], keyText: "", pad: [10], padText: "L3 / LS" },
  { label: "R3", keys: [], keyText: "", pad: [11], padText: "R3 / RS" },
  { label: "START", keys: ["Enter", "NumpadEnter"], keyText: "Return", pad: [9], padText: "Options / Menu" },
  { label: "SELECT", keys: ["Tab"], keyText: "Tab", pad: [8], padText: "Create, Share / View" },
  { label: "controls.dpadLs", keys: ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"], keyText: "↑ ↓ ← →", pad: [12, 13, 14, 15], padText: "D-pad" },
  { label: "controls.ls", keys: [], keyText: "", pad: [], padText: "controls.stickL" },
  { label: "controls.rs", keys: [], keyText: "", pad: [], padText: "controls.stickR" },
];

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const visible = () => $("view-controls").classList.contains("active");
const heldKeys = new Set<string>();
let heldPad = new Set<number>();
let padFrame = 0;

const text = (v: string) => {
  const s = t(v as Key);
  return s === v ? v : s;
};

export function renderControls() {
  const body = $("controls-body");
  body.replaceChildren();
  ROWS.forEach((r, i) => {
    const tr = document.createElement("tr");
    tr.dataset.row = String(i);
    const th = document.createElement("th");
    th.textContent = text(r.label);
    const kb = document.createElement("td");
    kb.textContent = r.keyText || "—";
    const pad = document.createElement("td");
    pad.textContent = text(r.padText);
    tr.append(th, kb, pad);
    body.append(tr);
  });
  renderHeld();
}

function renderHeld() {
  document.querySelectorAll<HTMLElement>("#controls-body tr").forEach((tr) => {
    const r = ROWS[Number(tr.dataset.row)];
    const on = r.keys.some((k) => heldKeys.has(k)) || r.pad.some((b) => heldPad.has(b));
    tr.classList.toggle("held", on);
  });
}

window.addEventListener(
  "keydown",
  (ev) => {
    if (!visible() || ev.metaKey) return; // Cmd shortcuts stay the app's
    if (ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement || ev.target instanceof HTMLTextAreaElement) return;
    if (!ROWS.some((r) => r.keys.includes(ev.code))) return;
    ev.preventDefault();
    heldKeys.add(ev.code);
    renderHeld();
  },
  true,
);
window.addEventListener(
  "keyup",
  (ev) => {
    if (heldKeys.delete(ev.code)) renderHeld();
  },
  true,
);
window.addEventListener("blur", () => {
  heldKeys.clear();
  renderHeld();
});

/** The first connected pad, if the web view has the Gamepad API. */
function firstPad(): Gamepad | null {
  const pads = typeof navigator.getGamepads === "function" ? navigator.getGamepads() : [];
  for (const p of pads) if (p && p.connected) return p;
  return null;
}

/** The controller test: its name and where its sticks are. */
function renderPad(pad: Gamepad | null) {
  $("pad-name").textContent = pad
    ? t("controls.connected", { name: pad.id })
    : typeof navigator.getGamepads === "function"
      ? t("controls.noPad")
      : t("controls.noApi");
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
  const pad = firstPad();
  renderPad(pad);
  const held = new Set<number>();
  pad?.buttons.forEach((b, i) => (b.pressed || b.value > 0.5) && held.add(i));
  if ([...held].join() !== [...heldPad].join()) {
    heldPad = held;
    renderHeld();
  }
  padFrame = requestAnimationFrame(pollPad);
}

/** Called when the Controls view opens or closes. */
export function showControls(on = true) {
  if (!on) return;
  renderControls();
  if (!padFrame) padFrame = requestAnimationFrame(pollPad);
}

export function initControls() {
  window.addEventListener("gamepadconnected", () => {
    if (visible() && !padFrame) padFrame = requestAnimationFrame(pollPad);
  });
}
