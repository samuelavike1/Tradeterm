//! Drawing templates (spec section 5): per tool, a remembered default style
//! (the last committed style, used by new drawings) and named templates.
//!
//! A template is a drawing's [`Props`] only, so it never carries points,
//! text content, lock, hidden or visibility. Props are kept as raw JSON and
//! read as an overlay on the tool's factory defaults when applied, so
//! templates saved by older or newer builds still apply.
//!
//! Loading is tolerant: an unknown tool (from a newer build) or an
//! unreadable entry is skipped but kept raw and written back on save.

use super::model::DrawingTool;
use super::props::Props;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A named template.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedTemplate {
    pub name: String,
    pub props: Value,
}

/// One tool's templates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolTemplates {
    /// The tool's default style (set by "Save as default" and by every
    /// committed style edit); `None` = factory defaults.
    pub default: Option<Value>,
    /// In the order saved.
    pub named: Vec<NamedTemplate>,
    /// Named entries this build can't read, kept as is.
    pub unknown: Vec<Value>,
}

impl ToolTemplates {
    fn is_empty(&self) -> bool {
        self.default.is_none() && self.named.is_empty() && self.unknown.is_empty()
    }

    fn from_value(v: Value) -> Option<Self> {
        let Value::Object(mut m) = v else {
            return None;
        };
        let default = m.remove("default").filter(|d| d.is_object());
        let mut t = ToolTemplates {
            default,
            ..Default::default()
        };
        if let Some(Value::Array(named)) = m.remove("named") {
            for e in named {
                let name = e.get("name").and_then(|n| n.as_str()).map(str::to_owned);
                match (name, e.get("props")) {
                    (Some(name), Some(props)) if props.is_object() && !name.is_empty() => {
                        t.named.push(NamedTemplate {
                            name,
                            props: props.clone(),
                        });
                    }
                    _ => {
                        tracing::warn!("drawing templates: skipping an unreadable template");
                        t.unknown.push(e);
                    }
                }
            }
        }
        Some(t)
    }

    fn to_value(&self) -> Value {
        let mut named: Vec<Value> = self
            .named
            .iter()
            .map(|n| serde_json::json!({ "name": n.name, "props": n.props }))
            .collect();
        named.extend(self.unknown.iter().cloned());
        serde_json::json!({ "default": self.default, "named": named })
    }
}

/// Every tool's templates (app-wide, like TradingView's account templates).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrawingTemplates {
    pub tools: BTreeMap<DrawingToolKey, ToolTemplates>,
    /// Entries for tools this build doesn't know, kept as is.
    pub unknown: Map<String, Value>,
}

/// [`DrawingTool`] as a map key (ordered by its saved name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawingToolKey(pub DrawingTool);

impl PartialOrd for DrawingToolKey {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for DrawingToolKey {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        tool_name(self.0).cmp(&tool_name(o.0))
    }
}

/// The name a tool is saved as (its variant name).
pub fn tool_name(t: DrawingTool) -> String {
    match serde_json::to_value(t) {
        Ok(Value::String(s)) => s,
        _ => format!("{t:?}"),
    }
}

/// The style payload of a template: the props as JSON.
pub fn template_value(props: &Props) -> Value {
    serde_json::to_value(props).unwrap_or(Value::Null)
}

impl DrawingTemplates {
    pub fn tool(&self, t: DrawingTool) -> Option<&ToolTemplates> {
        self.tools.get(&DrawingToolKey(t))
    }

    fn tool_mut(&mut self, t: DrawingTool) -> &mut ToolTemplates {
        self.tools.entry(DrawingToolKey(t)).or_default()
    }

    /// The tool's default style, if one is saved and readable.
    pub fn default_props(&self, t: DrawingTool) -> Option<Props> {
        let v = self.tool(t)?.default.as_ref()?;
        Props::overlay(t, v)
            .map_err(|e| tracing::warn!("drawing templates: default for {t:?}: {e}"))
            .ok()
    }

    /// What a new drawing of `t` starts from: the saved default, else the
    /// factory defaults.
    pub fn new_drawing_props(&self, t: DrawingTool) -> Props {
        self.default_props(t).unwrap_or_else(|| Props::defaults(t))
    }

    /// "Save as default" and the last-used style of every committed edit.
    pub fn set_default(&mut self, t: DrawingTool, props: &Props) {
        if props.kind() != Props::defaults(t).kind() {
            return;
        }
        self.tool_mut(t).default = Some(template_value(props));
    }

    /// "Reset": back to factory defaults for new drawings.
    pub fn clear_default(&mut self, t: DrawingTool) {
        if let Some(tt) = self.tools.get_mut(&DrawingToolKey(t)) {
            tt.default = None;
            if tt.is_empty() {
                self.tools.remove(&DrawingToolKey(t));
            }
        }
    }

    pub fn has_default(&self, t: DrawingTool) -> bool {
        self.tool(t).is_some_and(|x| x.default.is_some())
    }

    /// The tool's named templates, in the order saved.
    pub fn named(&self, t: DrawingTool) -> &[NamedTemplate] {
        self.tool(t).map_or(&[], |x| x.named.as_slice())
    }

    pub fn has_named(&self, t: DrawingTool, name: &str) -> bool {
        self.named(t).iter().any(|n| n.name == name)
    }

    /// "Save Drawing Template As…": adds or replaces `name`. Returns
    /// whether it replaced one.
    pub fn save_named(&mut self, t: DrawingTool, name: &str, props: &Props) -> bool {
        let name = name.trim();
        if name.is_empty() || props.kind() != Props::defaults(t).kind() {
            return false;
        }
        let value = template_value(props);
        let tt = self.tool_mut(t);
        match tt.named.iter_mut().find(|n| n.name == name) {
            Some(n) => {
                n.props = value;
                true
            }
            None => {
                tt.named.push(NamedTemplate {
                    name: name.to_owned(),
                    props: value,
                });
                false
            }
        }
    }

    /// A named template's style (read over the tool's defaults).
    pub fn named_props(&self, t: DrawingTool, name: &str) -> Option<Props> {
        let n = self.named(t).iter().find(|n| n.name == name)?;
        Props::overlay(t, &n.props)
            .map_err(|e| tracing::warn!("drawing template {name:?} for {t:?}: {e}"))
            .ok()
    }

    pub fn remove_named(&mut self, t: DrawingTool, name: &str) -> bool {
        let Some(tt) = self.tools.get_mut(&DrawingToolKey(t)) else {
            return false;
        };
        let n = tt.named.len();
        tt.named.retain(|x| x.name != name);
        let removed = tt.named.len() != n;
        if tt.is_empty() {
            self.tools.remove(&DrawingToolKey(t));
        }
        removed
    }

    /// Every readable default, for [`super::Drawings::set_tool_defaults`].
    pub fn defaults(&self) -> Vec<(DrawingTool, Props)> {
        self.tools
            .keys()
            .filter_map(|k| Some((k.0, self.default_props(k.0)?)))
            .collect()
    }

    /// Reads templates from JSON, never failing on content.
    pub fn from_value(v: Value) -> Self {
        let mut out = Self::default();
        let Value::Object(m) = v else {
            if !v.is_null() {
                tracing::warn!("drawing templates: expected a map, ignoring them");
            }
            return out;
        };
        for (k, v) in m {
            let tool = serde_json::from_value::<DrawingTool>(Value::String(k.clone())).ok();
            match tool {
                Some(t) => match ToolTemplates::from_value(v.clone()) {
                    Some(tt) => {
                        out.tools.insert(DrawingToolKey(t), tt);
                    }
                    None => {
                        tracing::warn!("drawing templates: unreadable entry for {k}");
                        out.unknown.insert(k, v);
                    }
                },
                None => {
                    out.unknown.insert(k, v);
                }
            }
        }
        out
    }

    pub fn to_value(&self) -> Value {
        let mut m = self.unknown.clone();
        for (k, tt) in &self.tools {
            if !tt.is_empty() {
                m.insert(tool_name(k.0), tt.to_value());
            }
        }
        Value::Object(m)
    }
}

impl Serialize for DrawingTemplates {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(s)
    }
}

impl<'de> Deserialize<'de> for DrawingTemplates {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::from_value(Value::deserialize(d)?))
    }
}
