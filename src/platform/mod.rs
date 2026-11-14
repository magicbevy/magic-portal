use std::path::Path;

pub mod linux;
pub mod macos;
pub mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Macos,
    Windows,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Linux => linux::NAME,
            Self::Macos => macos::NAME,
            Self::Windows => windows::NAME,
        }
    }

    pub fn spell_relative_path(self) -> &'static str {
        match self {
            Self::Linux => linux::SPELL_RELATIVE_PATH,
            Self::Macos => macos::SPELL_RELATIVE_PATH,
            Self::Windows => windows::SPELL_RELATIVE_PATH,
        }
    }

    pub fn spell_program(self) -> &'static str {
        match self {
            Self::Linux => linux::SPELL_PROGRAM,
            Self::Macos => macos::SPELL_PROGRAM,
            Self::Windows => windows::SPELL_PROGRAM,
        }
    }

    pub fn spell_args(self, spell: &Path) -> Vec<String> {
        match self {
            Self::Linux => linux::spell_args(spell),
            Self::Macos => macos::spell_args(spell),
            Self::Windows => windows::spell_args(spell),
        }
    }

    pub fn cargo_config_relative_path(self) -> &'static str {
        match self {
            Self::Linux => linux::CARGO_CONFIG_RELATIVE_PATH,
            Self::Macos => macos::CARGO_CONFIG_RELATIVE_PATH,
            Self::Windows => windows::CARGO_CONFIG_RELATIVE_PATH,
        }
    }

    pub fn executable_name(self, name: &str) -> String {
        match self {
            Self::Windows => format!("{name}.exe"),
            Self::Linux | Self::Macos => name.to_string(),
        }
    }

    pub fn archive_extension(self) -> &'static str {
        match self {
            Self::Linux | Self::Macos => "tar.gz",
            Self::Windows => "zip",
        }
    }
}

pub fn architecture() -> &'static str {
    std::env::consts::ARCH
}
