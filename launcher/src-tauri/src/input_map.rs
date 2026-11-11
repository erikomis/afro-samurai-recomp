//! The controls file the game reads through `PS3_INPUT_MAP` (format in the
//! toolkit's `libs/input/pad_input_map.h`): `[keyboard]`, `[mouse_buttons]`
//! and `[controller]` bind PS3 controls to keys, mouse buttons and SDL
//! controller inputs; `[mouse]` holds the camera settings.
//!
//! The runtime's built-in layout is a first-person one (WASD, the mouse
//! camera), which is not this port's: the port's own is `port/input.toml`
//! (`port_defaults` here), the keyboard as a pad with no mouse. So the file
//! the launcher writes is always the full layout: every control of the three
//! binding sections (the port's defaults with the player's changes on top)
//! and a `[mouse]` section from the launcher's mouse settings. Any other
//! section stays as it was found.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// PS3 control -> inputs, per device; an empty list is "unbound".
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Bindings {
    pub keyboard: BTreeMap<String, Vec<String>>,
    pub mouse_buttons: BTreeMap<String, Vec<String>>,
    pub controller: BTreeMap<String, Vec<String>>,
}

/// `[mouse]`: the mouse camera's settings (the launcher's, see `launch_env`).
#[derive(Clone, Debug, PartialEq)]
pub struct Mouse {
    pub camera: bool,
    pub sensitivity: f32,
    pub invert_x: bool,
    pub invert_y: bool,
    pub deadzone: u32,
    pub capture: bool,
}

/// A parsed file: the bindings, `[mouse]` when it has one (a key the section
/// leaves out reads as the runtime's default), and the sections the launcher
/// does not know, kept verbatim.
#[derive(Debug, Default, PartialEq)]
pub struct InputFile {
    pub bindings: Bindings,
    pub mouse: Option<Mouse>,
    pub other: String,
}

/// The order controls are written in (the file accepts any).
pub const CONTROLS: &[&str] = &[
    "cross", "circle", "square", "triangle", "l1", "r1", "l2", "r2", "l3", "r3", "start", "select",
    "up", "down", "left", "right", "ls_up", "ls_down", "ls_left", "ls_right", "rs_up", "rs_down",
    "rs_left", "rs_right",
];

/// The runtime's `[mouse]` defaults (cellPad.c): what a key the file leaves
/// out means.
const RUNTIME_MOUSE: Mouse = Mouse { camera: true, sensitivity: 1.0, invert_x: false, invert_y: false, deadzone: 30, capture: true };

/// This port's layout, the one in the checked-in `port/input.toml`: the
/// keyboard as a pad (the arrows are the d-pad and the left stick), no mouse
/// buttons, and SDL's standard mapping for a controller (the runtime's own).
/// Every control is listed, the unbound ones with an empty list.
pub fn port_defaults() -> Bindings {
    const KEYBOARD: &[(&str, &str)] = &[
        ("cross", "Z"), ("circle", "X"), ("square", "A"), ("triangle", "S"),
        ("l1", "Q"), ("r1", "W"), ("l2", "1"), ("r2", "2"),
        ("start", "Return"), ("select", "Tab"),
        ("up", "Up"), ("down", "Down"), ("left", "Left"), ("right", "Right"),
        ("ls_up", "Up"), ("ls_down", "Down"), ("ls_left", "Left"), ("ls_right", "Right"),
    ];
    const CONTROLLER: &[(&str, &str)] = &[
        ("cross", "a"), ("circle", "b"), ("square", "x"), ("triangle", "y"),
        ("l1", "leftshoulder"), ("r1", "rightshoulder"), ("l2", "lefttrigger"), ("r2", "righttrigger"),
        ("l3", "leftstick"), ("r3", "rightstick"), ("start", "start"), ("select", "back"),
        ("up", "dpup"), ("down", "dpdown"), ("left", "dpleft"), ("right", "dpright"),
    ];
    let fill = |pairs: &[(&str, &str)]| -> BTreeMap<String, Vec<String>> {
        let mut m: BTreeMap<String, Vec<String>> = CONTROLS.iter().map(|c| (c.to_string(), Vec::new())).collect();
        for (c, input) in pairs {
            m.insert(c.to_string(), vec![input.to_string()]);
        }
        m
    };
    Bindings { keyboard: fill(KEYBOARD), mouse_buttons: fill(&[]), controller: fill(CONTROLLER) }
}

/// The port's defaults with `b`'s entries on top: the full layout, as the
/// Controls page shows it and as the file is written.
pub fn full(b: &Bindings) -> Bindings {
    let mut out = port_defaults();
    out.keyboard.extend(b.keyboard.clone());
    out.mouse_buttons.extend(b.mouse_buttons.clone());
    out.controller.extend(b.controller.clone());
    out
}

#[derive(PartialEq, Clone, Copy)]
enum Sec {
    Preamble,
    Keyboard,
    MouseButtons,
    Controller,
    Mouse,
    Other,
}

fn section_of(name: &str) -> Sec {
    match name.trim().to_ascii_lowercase().as_str() {
        "keyboard" | "keys" => Sec::Keyboard,
        "mouse_buttons" | "mouse.buttons" => Sec::MouseButtons,
        "controller" | "gamepad" | "pad" => Sec::Controller,
        "mouse" => Sec::Mouse,
        _ => Sec::Other,
    }
}

/// Cut a line's comment ('#' or ';' outside quotes).
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' || c == ';' => return &line[..i],
            None => {}
        }
    }
    line
}

/// `"a, b"`, `['a', "b"]` or `a,b` -> ["a", "b"]; "" and "none" are no input.
fn split_list(value: &str) -> Vec<String> {
    let flat: String = value.chars().filter(|c| !matches!(c, '"' | '\'' | '[' | ']')).collect();
    flat.split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("none"))
        .map(String::from)
        .collect()
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// One `key = value` of `[mouse]`; an unknown key or value changes nothing.
fn set_mouse(m: &mut Mouse, key: &str, value: &str) {
    let v: String = value.chars().filter(|c| !matches!(c, '"' | '\'')).collect();
    let v = v.trim();
    match key {
        "camera" => m.camera = parse_bool(v).unwrap_or(m.camera),
        "capture" => m.capture = parse_bool(v).unwrap_or(m.capture),
        "invert_x" => m.invert_x = parse_bool(v).unwrap_or(m.invert_x),
        "invert_y" => m.invert_y = parse_bool(v).unwrap_or(m.invert_y),
        "sensitivity" => m.sensitivity = v.parse().ok().filter(|x: &f32| *x > 0.0).unwrap_or(m.sensitivity),
        "deadzone" => m.deadzone = v.parse().unwrap_or(m.deadzone),
        _ => {}
    }
}

pub fn parse(text: &str) -> InputFile {
    let mut out = InputFile::default();
    let mut sec = Sec::Preamble;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if let Some(rest) = line.strip_prefix('[') {
            let name = rest.split(']').next().unwrap_or("");
            sec = section_of(name);
            if sec == Sec::Mouse {
                out.mouse.get_or_insert(RUNTIME_MOUSE);
            }
            if sec == Sec::Other {
                if !out.other.is_empty() && !out.other.ends_with("\n\n") {
                    out.other.push('\n');
                }
                out.other.push_str(raw.trim_end());
                out.other.push('\n');
            }
            continue;
        }
        if sec == Sec::Preamble {
            continue; // the launcher writes its own header
        }
        if sec == Sec::Other {
            out.other.push_str(raw.trim_end());
            out.other.push('\n');
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let key: String = k.chars().filter(|c| !matches!(c, '"' | '\'')).collect();
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        let map = match sec {
            Sec::Keyboard => &mut out.bindings.keyboard,
            Sec::MouseButtons => &mut out.bindings.mouse_buttons,
            Sec::Controller => &mut out.bindings.controller,
            _ => {
                if let Some(m) = out.mouse.as_mut() {
                    set_mouse(m, &key, v);
                }
                continue;
            }
        };
        map.insert(key, split_list(v));
    }
    while out.other.ends_with("\n\n") {
        out.other.pop();
    }
    out
}

/// Names go into the file between quotes: only plain identifiers.
fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 32 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn validate(b: &Bindings) -> Result<(), String> {
    for map in [&b.keyboard, &b.mouse_buttons, &b.controller] {
        for (k, v) in map {
            if !valid_name(k) {
                return Err(format!("{k:?}: not a control name"));
            }
            if let Some(bad) = v.iter().find(|n| !valid_name(n)) {
                return Err(format!("{bad:?}: not an input name"));
            }
        }
    }
    Ok(())
}

fn write_section(out: &mut String, title: &str, map: &BTreeMap<String, Vec<String>>) {
    if map.is_empty() {
        return;
    }
    out.push_str(&format!("\n[{title}]\n"));
    let known = CONTROLS.iter().filter_map(|c| map.get_key_value(*c));
    let extra = map.iter().filter(|(k, _)| !CONTROLS.contains(&k.as_str()));
    for (k, v) in known.chain(extra) {
        out.push_str(&format!("{k:<8} = \"{}\"\n", v.join(", ")));
    }
}

pub fn render(f: &InputFile) -> String {
    let mut out = String::from(
        "# Afro Samurai recomp: controls, read by the game through PS3_INPUT_MAP.\n\
         # Written by the launcher (Controls) in full: every control is listed (\"\" =\n\
         # unbound), and [mouse] follows the launcher's mouse settings. Hand edits are\n\
         # fine, but comments in these sections are not kept, and a control left out\n\
         # gets the port's default back at the next save. Names: launcher/README.md.\n",
    );
    write_section(&mut out, "keyboard", &f.bindings.keyboard);
    write_section(&mut out, "mouse_buttons", &f.bindings.mouse_buttons);
    write_section(&mut out, "controller", &f.bindings.controller);
    if let Some(m) = &f.mouse {
        out.push_str(&format!(
            "\n[mouse]\ncamera      = {}\nsensitivity = {:.2}\ninvert_x    = {}\ninvert_y    = {}\ndeadzone    = {}\ncapture     = {}\n",
            m.camera, m.sensitivity, m.invert_x, m.invert_y, m.deadzone, m.capture
        ));
    }
    if !f.other.trim().is_empty() {
        out.push('\n');
        out.push_str(f.other.trim_end());
        out.push('\n');
    }
    out
}

/// What "Play" passes for the file: its path, and whether it has a `[mouse]`
/// section the launcher's settings must override.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct InputEnv {
    pub path: Option<String>,
    pub has_mouse_section: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_forms_and_keep_other_sections() {
        let f = parse(
            "# header\n\
             [keyboard]\n\
             Cross = \"Space\"   # jump\n\
             start = 'Return, KeypadEnter'\n\
             select = [\"Tab\", \"Backspace\"]\n\
             l3 = \"\"\n\
             r3 = none\n\
             \n\
             [mouse]\n\
             sensitivity = 1.5 # fast\n\
             invert_y = true\n\
             nonsense = 3\n\
             [mouse_buttons]\n\
             r1 = Left\n\
             [Controller]\n\
             cross = \"b\"\n\
             [future]\n\
             x = 1\n",
        );
        let k = &f.bindings.keyboard;
        assert_eq!(k["cross"], vec!["Space"]);
        assert_eq!(k["start"], vec!["Return", "KeypadEnter"]);
        assert_eq!(k["select"], vec!["Tab", "Backspace"]);
        assert!(k["l3"].is_empty() && k["r3"].is_empty());
        assert_eq!(f.bindings.mouse_buttons["r1"], vec!["Left"]);
        assert_eq!(f.bindings.controller["cross"], vec!["b"]);
        // [mouse]: the keys it has, the runtime's defaults for the rest
        assert_eq!(f.mouse, Some(Mouse { sensitivity: 1.5, invert_y: true, ..RUNTIME_MOUSE }));
        assert_eq!(f.other, "[future]\nx = 1\n");
        assert_eq!(parse("[keyboard]\ncross = \"Z\"\n").mouse, None);
    }

    #[test]
    fn render_round_trips() {
        let mut f = InputFile::default();
        f.bindings.keyboard.insert("start".into(), vec!["Return".into(), "KeypadEnter".into()]);
        f.bindings.keyboard.insert("cross".into(), vec!["F".into()]);
        f.bindings.keyboard.insert("l3".into(), vec![]);
        f.bindings.controller.insert("r1".into(), vec!["righttrigger".into()]);
        f.mouse = Some(Mouse { camera: false, sensitivity: 2.5, invert_x: true, invert_y: false, deadzone: 12, capture: false });
        f.other = "[future]\nx = 1\n".into();
        let text = render(&f);
        assert!(text.starts_with("# Afro Samurai recomp: controls"));
        // cross before start (the fixed order), the empty list as ""
        let cross = text.find("cross    = \"F\"").unwrap();
        let start = text.find("start    = \"Return, KeypadEnter\"").unwrap();
        assert!(cross < start);
        assert!(text.contains("l3       = \"\""));
        assert!(!text.contains("\n[mouse_buttons]\n")); // nothing to say
        assert!(text.contains("[mouse]\ncamera      = false\nsensitivity = 2.50\ninvert_x    = true\ninvert_y    = false\ndeadzone    = 12\ncapture     = false\n"));
        assert_eq!(parse(&text), f);
    }

    #[test]
    fn names_are_checked() {
        let mut b = Bindings::default();
        b.keyboard.insert("cross".into(), vec!["Space".into(), "0x0A".into()]);
        assert!(validate(&b).is_ok());
        b.keyboard.insert("circle".into(), vec!["Q\"\n[x]".into()]);
        assert!(validate(&b).is_err());
        let mut b = Bindings::default();
        b.controller.insert("bad key".into(), vec![]);
        assert!(validate(&b).is_err());
    }

    /// The defaults are the checked-in port/input.toml: its keyboard, no
    /// mouse camera; and they list every control of every device.
    #[test]
    fn port_defaults_are_the_checked_in_file() {
        let d = port_defaults();
        for map in [&d.keyboard, &d.mouse_buttons, &d.controller] {
            assert_eq!(map.keys().map(String::as_str).collect::<std::collections::BTreeSet<_>>(), CONTROLS.iter().copied().collect());
        }
        assert!(d.mouse_buttons.values().all(Vec::is_empty));
        assert!(d.keyboard["l3"].is_empty() && d.keyboard["rs_up"].is_empty());
        assert_eq!((d.keyboard["up"].clone(), d.keyboard["ls_up"].clone()), (vec!["Up".to_string()], vec!["Up".to_string()]));
        assert_eq!(d.controller["r2"], vec!["righttrigger"]);
        let f = parse(include_str!("../../../port/input.toml"));
        assert_eq!(f.bindings.keyboard, d.keyboard);
        assert!(f.bindings.mouse_buttons.is_empty() && f.bindings.controller.is_empty());
        assert_eq!(f.mouse, Some(Mouse { camera: false, ..RUNTIME_MOUSE }));
        assert!(validate(&d).is_ok());
    }

    #[test]
    fn full_is_the_defaults_with_the_changes_on_top() {
        let mut b = Bindings::default();
        b.keyboard.insert("cross".into(), vec!["Space".into()]);
        b.keyboard.insert("start".into(), vec![]);
        b.mouse_buttons.insert("r1".into(), vec!["Left".into()]);
        let f = full(&b);
        assert_eq!(f.keyboard["cross"], vec!["Space"]);
        assert!(f.keyboard["start"].is_empty());
        assert_eq!(f.keyboard["circle"], vec!["X"]);
        assert_eq!(f.mouse_buttons["r1"], vec!["Left"]);
        assert!(f.mouse_buttons["l1"].is_empty());
        assert_eq!(f.controller, port_defaults().controller);
        assert_eq!(full(&Bindings::default()), port_defaults());
    }
}
