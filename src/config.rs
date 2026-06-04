use clap::ValueEnum;

pub(crate) const PLATFORM: &str = "x64";

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Config {
    Debug,
    Release,
}

impl Config {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}
