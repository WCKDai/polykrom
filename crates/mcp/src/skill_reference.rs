//! Read-only access to the skill documents shipped with SolidCraft.

use serde_json::{Map, Value};

struct Skill {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
}

// Exact names only: these are embedded documents, never filesystem paths to open.
const SKILLS: &[Skill] = &[
    Skill {
        name: "solidcraft-checking",
        files: &[
            ("SKILL.md", include_str!("../../../skills/solidcraft-checking/SKILL.md")),
            ("reference/porting.md", include_str!("../../../skills/solidcraft-checking/reference/porting.md")),
        ],
    },
    Skill { name: "solidcraft-editing", files: &[("SKILL.md", include_str!("../../../skills/solidcraft-editing/SKILL.md"))] },
    Skill {
        name: "solidcraft-mechanisms",
        files: &[
            ("SKILL.md", include_str!("../../../skills/solidcraft-mechanisms/SKILL.md")),
            ("reference.md", include_str!("../../../skills/solidcraft-mechanisms/reference.md")),
        ],
    },
    Skill {
        name: "solidcraft-modeling",
        files: &[
            ("SKILL.md", include_str!("../../../skills/solidcraft-modeling/SKILL.md")),
            ("reference.md", include_str!("../../../skills/solidcraft-modeling/reference.md")),
        ],
    },
    Skill { name: "solidcraft-polish", files: &[("SKILL.md", include_str!("../../../skills/solidcraft-polish/SKILL.md"))] },
];

/// A skill's main document: its `reference.md`, or `SKILL.md` for a skill without one (#272).
fn main_file(skill: &Skill) -> &'static (&'static str, &'static str) {
    skill.files.iter().find(|(name, _)| *name == "reference.md").or_else(|| skill.files.first()).unwrap_or(&("SKILL.md", ""))
}

pub(crate) fn read(args: &Map<String, Value>) -> Result<Value, String> {
    let name = args.get("skill").and_then(Value::as_str).ok_or("`skill` is required and must be a string")?;
    let skill = SKILLS
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("Unknown skill. Available skills: {}", SKILLS.iter().map(|s| s.name).collect::<Vec<_>>().join(", ")))?;
    let available = || skill.files.iter().map(|(name, _)| *name).collect::<Vec<_>>().join(", ");
    let file = match args.get("file") {
        None | Some(Value::Null) => return Ok(Value::String(main_file(skill).1.into())),
        Some(Value::String(file)) => file.as_str(),
        Some(_) => return Err("`file` must be a string".into()),
    };
    if let Some((_, text)) = skill.files.iter().find(|(name, _)| *name == file) {
        return Ok(Value::String((*text).into()));
    }
    // A path out of the skill is refused; any other name gets the closest document, saying so,
    // rather than a failed call (#272): the one of that file name in a subfolder, else the main one.
    if file.contains("..") || file.starts_with('/') || file.contains('\\') {
        return Err(format!("Unknown file for {}. Available files: {}", skill.name, available()));
    }
    let base = file.rsplit('/').next().unwrap_or(file);
    let (shown, text) = skill.files.iter().find(|(name, _)| name.rsplit('/').next() == Some(base)).unwrap_or_else(|| main_file(skill));
    Ok(Value::String(format!("(No `{file}` in {}; this is {shown}. Available files: {}.)\n\n{text}", skill.name, available())))
}
