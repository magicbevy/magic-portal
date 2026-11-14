use std::path::Path;

pub const NAME: &str = "windows";
pub const SPELL_RELATIVE_PATH: &str = "portal/spells/windows.ps1";
pub const SPELL_PROGRAM: &str = "powershell";
pub const CARGO_CONFIG_RELATIVE_PATH: &str = "portal/config/windows/cargo.toml";

pub fn spell_args(spell: &Path) -> Vec<String> {
    vec![
        "-NoProfile".into(),
        "-ExecutionPolicy".into(),
        "Bypass".into(),
        "-File".into(),
        spell.to_string_lossy().into_owned(),
    ]
}
