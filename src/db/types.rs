#[derive(Debug, Clone, Copy)]
pub enum UnitMake {
    TELTONIKA,
    RUPTELA,
}

impl UnitMake {
    pub(crate) fn from_db(s: &str) -> Option<Self> {
        Self::from_subject_segment(s)
    }

    pub fn from_subject_segment(s: &str) -> Option<Self> {
        match s {
            "TELTONIKA" => Some(UnitMake::TELTONIKA),
            "RUPTELA" => Some(UnitMake::RUPTELA),
            _ => None,
        }
    }
}
