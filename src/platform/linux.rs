use std::path::Path;

pub const NAME: &str = "linux";
pub const SPELL_RELATIVE_PATH: &str = "portal/spells/linux.sh";
pub const SPELL_PROGRAM: &str = "bash";
pub const CARGO_CONFIG_RELATIVE_PATH: &str = "portal/config/linux/cargo.toml";

pub fn spell_args(spell: &Path) -> Vec<String> {
    vec![spell.to_string_lossy().into_owned()]
}
