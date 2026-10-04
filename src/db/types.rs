#[derive(Debug, Clone, Copy)]
pub enum UnitMake {
    Teltonika,
    Ruptela,
}

impl UnitMake {
    pub(crate) fn from_db(s: &str) -> Option<Self> {
        Self::from_subject_segment(s)
    }

    pub fn from_subject_segment(s: &str) -> Option<Self> {
        match s {
            "TELTONIKA" => Some(UnitMake::Teltonika),
            "RUPTELA" => Some(UnitMake::Ruptela),
            _ => None,
        }
    }

    pub fn as_subject_segment(self) -> &'static str {
        match self {
            UnitMake::Teltonika => "TELTONIKA",
            UnitMake::Ruptela => "RUPTELA",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UnitMake;

    #[test]
    fn preserves_uppercase_subject_segments() {
        assert_eq!(UnitMake::Teltonika.as_subject_segment(), "TELTONIKA");
        assert_eq!(UnitMake::Ruptela.as_subject_segment(), "RUPTELA");
    }
}
