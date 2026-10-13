//! Install the Clawdmeter hooks into each account's `settings.json`.
//!
//! Claude Code reads `settings.json` from each config dir separately, so the
//! installer runs per account. It merges into existing hooks, produces the
//! before/after text for a diff, keeps a `.bak`, and only ever touches hook
//! entries whose command runs `attention-hook.sh` (ours). Hooks the user or
//! other tools added are left exactly as they are.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

/// The hook script's file name; any command containing it is ours.
pub const HOOK_SCRIPT: &str = "attention-hook.sh";

/// One hook the app installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookSpec {
    /// Hook event name.
    pub event: &'static str,
    /// Tool matcher, for PreToolUse.
    pub matcher: Option<&'static str>,
    /// Script arguments.
    pub args: &'static str,
}

/// The five hooks (spec §08): four from the attention feature plus Stop, all
/// also logging to events.log.
pub const HOOKS: [HookSpec; 5] = [
    HookSpec { event: "UserPromptSubmit", matcher: None, args: "clear event" },
    HookSpec { event: "PostToolUse", matcher: None, args: "clear event" },
    HookSpec { event: "Stop", matcher: None, args: "event" },
    HookSpec { event: "Notification", matcher: None, args: "alert event" },
    HookSpec { event: "PreToolUse", matcher: Some("AskUserQuestion"), args: "alert event" },
];

/// Hook problems.
#[derive(Debug)]
pub enum HookError {
    /// settings.json isn't valid JSON (left untouched).
    Parse(String),
    /// settings.json (or its `hooks`) has an unexpected shape.
    Shape(&'static str),
    /// File I/O.
    Io(io::Error),
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookError::Parse(e) => write!(f, "settings.json is not valid JSON ({e}); not touching it"),
            HookError::Shape(w) => write!(f, "settings.json: {w}; not touching it"),
            HookError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for HookError {}

impl From<io::Error> for HookError {
    fn from(e: io::Error) -> Self {
        HookError::Io(e)
    }
}

/// The command line for one hook.
pub fn hook_command(script: &Path, args: &str) -> String {
    let p = script.to_string_lossy();
    if p.contains(' ') {
        format!("\"{p}\" {args}")
    } else {
        format!("{p} {args}")
    }
}

fn is_ours(hook: &Value) -> bool {
    hook.get("command").and_then(Value::as_str).is_some_and(|c| c.contains(HOOK_SCRIPT))
}

fn group_is_ours(group: &Value) -> bool {
    group.get("hooks").and_then(Value::as_array).is_some_and(|hs| hs.iter().any(is_ours))
}

fn parse_root(text: Option<&str>) -> Result<Map<String, Value>, HookError> {
    match text.map(str::trim) {
        None | Some("") => Ok(Map::new()),
        Some(t) => match serde_json::from_str::<Value>(t).map_err(|e| HookError::Parse(e.to_string()))? {
            Value::Object(m) => Ok(m),
            _ => Err(HookError::Shape("top level is not an object")),
        },
    }
}

fn render(root: Map<String, Value>) -> String {
    let mut s = serde_json::to_string_pretty(&Value::Object(root)).expect("serializes");
    s.push('\n');
    s
}

/// Merge our hooks into settings.json text (`None` = no file). Returns the new
/// text; equal to the input's meaning when nothing needed to change.
pub fn merge(text: Option<&str>, script: &Path) -> Result<String, HookError> {
    let mut root = parse_root(text)?;
    let hooks = root.entry("hooks").or_insert_with(|| Value::Object(Map::new()));
    let hooks = hooks.as_object_mut().ok_or(HookError::Shape("\"hooks\" is not an object"))?;
    for spec in HOOKS {
        let cmd = hook_command(script, spec.args);
        let list = hooks.entry(spec.event).or_insert_with(|| Value::Array(Vec::new()));
        let list = list.as_array_mut().ok_or(HookError::Shape("a hook event is not an array"))?;
        let mut found = false;
        for group in list.iter_mut().filter(|g| group_is_ours(g)) {
            found = true;
            if let Some(m) = spec.matcher {
                group["matcher"] = Value::String(m.to_owned());
            }
            for h in group["hooks"].as_array_mut().expect("checked").iter_mut().filter(|h| is_ours(h)) {
                h["command"] = Value::String(cmd.clone());
            }
        }
        if !found {
            let mut g = Map::new();
            if let Some(m) = spec.matcher {
                g.insert("matcher".into(), Value::String(m.to_owned()));
            }
            g.insert("hooks".into(), json!([{ "type": "command", "command": cmd }]));
            list.push(Value::Object(g));
        }
    }
    Ok(render(root))
}

/// Remove our hook entries (and groups/events left empty by that).
pub fn remove(text: Option<&str>) -> Result<String, HookError> {
    let mut root = parse_root(text)?;
    if let Some(Value::Object(hooks)) = root.get_mut("hooks") {
        for list in hooks.values_mut() {
            if let Value::Array(groups) = list {
                for g in groups.iter_mut() {
                    if let Some(Value::Array(hs)) = g.get_mut("hooks") {
                        hs.retain(|h| !is_ours(h));
                    }
                }
                groups.retain(|g| g.get("hooks").and_then(Value::as_array).is_none_or(|hs| !hs.is_empty()));
            }
        }
        hooks.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
        if hooks.is_empty() {
            root.remove("hooks");
        }
    }
    Ok(render(root))
}

/// How much of our setup a settings.json has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookStatus {
    /// None of our hooks.
    Missing,
    /// Some events wired, or wired with old arguments (e.g. no `event`).
    Partial,
    /// All five, current arguments.
    Installed,
}

/// Check a settings.json against [`HOOKS`] (any script path counts).
pub fn status(text: Option<&str>) -> HookStatus {
    let Ok(root) = parse_root(text) else { return HookStatus::Missing };
    let hooks = root.get("hooks").and_then(Value::as_object);
    let mut have = 0;
    let mut current = 0;
    for spec in HOOKS {
        let ours: Vec<&str> = hooks
            .and_then(|h| h.get(spec.event))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|g| spec.matcher.is_none_or(|m| g.get("matcher").and_then(Value::as_str) == Some(m)))
            .filter_map(|g| g.get("hooks").and_then(Value::as_array))
            .flatten()
            .filter(|h| is_ours(h))
            .filter_map(|h| h.get("command").and_then(Value::as_str))
            .collect();
        if !ours.is_empty() {
            have += 1;
            if ours.iter().any(|c| c.trim_end().ends_with(spec.args)) {
                current += 1;
            }
        }
    }
    match (have, current) {
        (0, _) => HookStatus::Missing,
        (h, c) if h == HOOKS.len() && c == HOOKS.len() => HookStatus::Installed,
        _ => HookStatus::Partial,
    }
}

/// A pending change to one account's settings.json, for the diff view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookPlan {
    /// `<config_dir>/settings.json`.
    pub path: PathBuf,
    /// Current text (`None` = no file).
    pub before: Option<String>,
    /// Text after the change.
    pub after: String,
}

impl HookPlan {
    /// Whether applying would change anything (ignoring formatting).
    pub fn changes(&self) -> bool {
        let parse = |s: &str| serde_json::from_str::<Value>(s).ok();
        match &self.before {
            None => true,
            Some(b) => parse(b) != parse(&self.after),
        }
    }

    /// Write it: the old file goes to `settings.json.bak` first.
    pub fn apply(&self) -> io::Result<()> {
        if !self.changes() {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        if let Some(b) = &self.before {
            fs::write(self.path.with_extension("json.bak"), b)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, &self.after)?;
        fs::rename(&tmp, &self.path)
    }
}

fn read_opt(path: &Path) -> Result<Option<String>, HookError> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Plan installing the hooks for one config dir.
pub fn plan_install(config_dir: &Path, script: &Path) -> Result<HookPlan, HookError> {
    let path = config_dir.join("settings.json");
    let before = read_opt(&path)?;
    let after = merge(before.as_deref(), script)?;
    Ok(HookPlan { path, before, after })
}

/// Plan removing the hooks from one config dir.
pub fn plan_uninstall(config_dir: &Path) -> Result<HookPlan, HookError> {
    let path = config_dir.join("settings.json");
    let before = read_opt(&path)?;
    let after = remove(before.as_deref())?;
    Ok(HookPlan { path, before, after })
}

/// Hook status of one config dir.
pub fn dir_status(config_dir: &Path) -> HookStatus {
    status(read_opt(&config_dir.join("settings.json")).ok().flatten().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "/opt/clawdmeter/attention-hook.sh";

    #[test]
    fn fresh_install() {
        let out = merge(None, Path::new(SCRIPT)).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], format!("{SCRIPT} event"));
        assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "AskUserQuestion");
        assert_eq!(v["hooks"]["PostToolUse"][0]["hooks"][0]["type"], "command");
        assert_eq!(status(Some(&out)), HookStatus::Installed);
        assert_eq!(status(None), HookStatus::Missing);
    }

    #[test]
    fn merges_without_touching_others_and_upgrades_ours() {
        let before = r#"{
  "model": "opus",
  "permissions": {"allow": ["Bash(ls:*)"]},
  "hooks": {
    "PostToolUse": [
      {"matcher": "Edit", "hooks": [{"type": "command", "command": "prettier --write"}]},
      {"hooks": [{"type": "command", "command": "/home/u/Clawdmeter/daemon/attention-hook.sh clear"}]}
    ],
    "Notification": [{"hooks": [{"type": "command", "command": "/home/u/Clawdmeter/daemon/attention-hook.sh alert"}]}]
  }
}"#;
        assert_eq!(status(Some(before)), HookStatus::Partial);
        let out = merge(Some(before), Path::new(SCRIPT)).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        // Key order and unrelated settings survive.
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["model", "permissions", "hooks"]);
        assert_eq!(v["permissions"]["allow"][0], "Bash(ls:*)");
        let post = v["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(post.len(), 2, "upgraded in place, no duplicate group");
        assert_eq!(post[0]["hooks"][0]["command"], "prettier --write");
        assert_eq!(post[1]["hooks"][0]["command"], format!("{SCRIPT} clear event"));
        assert_eq!(v["hooks"]["Notification"].as_array().unwrap().len(), 1);
        assert_eq!(status(Some(&out)), HookStatus::Installed);
        // Idempotent.
        assert_eq!(merge(Some(&out), Path::new(SCRIPT)).unwrap(), out);
    }

    #[test]
    fn remove_only_ours() {
        let before = r#"{"hooks": {"PostToolUse": [
            {"matcher": "Edit", "hooks": [{"type": "command", "command": "prettier --write"}]},
            {"hooks": [{"type": "command", "command": "/x/attention-hook.sh clear event"}]}],
          "Stop": [{"hooks": [{"type": "command", "command": "/x/attention-hook.sh event"}]}]}}"#;
        let out = remove(Some(before)).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
        assert!(v["hooks"].get("Stop").is_none());
        let only_ours = r#"{"a": 1, "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "/x/attention-hook.sh event"}]}]}}"#;
        assert_eq!(remove(Some(only_ours)).unwrap(), "{\n  \"a\": 1\n}\n");
    }

    #[test]
    fn refuses_bad_files() {
        assert!(matches!(merge(Some("{not json"), Path::new(SCRIPT)), Err(HookError::Parse(_))));
        assert!(matches!(merge(Some("[]"), Path::new(SCRIPT)), Err(HookError::Shape(_))));
        assert!(matches!(merge(Some(r#"{"hooks": []}"#), Path::new(SCRIPT)), Err(HookError::Shape(_))));
    }

    #[test]
    fn quoting() {
        assert_eq!(hook_command(Path::new("/a b/attention-hook.sh"), "event"), "\"/a b/attention-hook.sh\" event");
    }

    #[test]
    fn plan_apply_with_backup() {
        let dir = std::env::temp_dir().join(format!("cm-hooks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.json"), "{\"model\":\"opus\"}").unwrap();
        let plan = plan_install(&dir, Path::new(SCRIPT)).unwrap();
        assert!(plan.changes());
        plan.apply().unwrap();
        assert_eq!(fs::read_to_string(dir.join("settings.json.bak")).unwrap(), "{\"model\":\"opus\"}");
        assert_eq!(dir_status(&dir), HookStatus::Installed);
        let again = plan_install(&dir, Path::new(SCRIPT)).unwrap();
        assert!(!again.changes());
        plan_uninstall(&dir).unwrap().apply().unwrap();
        assert_eq!(dir_status(&dir), HookStatus::Missing);
        let _ = fs::remove_dir_all(&dir);
    }
}
