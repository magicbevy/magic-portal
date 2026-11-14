use std::path::Path;

pub const NAME: &str = "macos";
pub const SPELL_RELATIVE_PATH: &str = "portal/spells/macos.sh";
pub const SPELL_PROGRAM: &str = "bash";
pub const CARGO_CONFIG_RELATIVE_PATH: &str = "portal/config/macos/cargo.toml";

pub fn spell_args(spell: &Path) -> Vec<String> {
    vec![spell.to_string_lossy().into_owned()]
}
