use std::{error::Error, fmt, io};

use crate::units::teltonika::validations::FutureTimestampError;

#[derive(Debug)]
pub enum TeltonikaFrameError {
    Parse(io::Error),
    Discarded {
        ack_record_count: u8,
        reason: DiscardReason,
    },
}

#[derive(Debug)]
pub enum DiscardReason {
    FutureTimestamp(FutureTimestampError),
}

impl TeltonikaFrameError {
    pub fn ack_record_count(&self) -> Option<u8> {
        match self {
            Self::Parse(_) => None,
            Self::Discarded {
                ack_record_count, ..
            } => Some(*ack_record_count),
        }
    }
}

impl fmt::Display for TeltonikaFrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "{err}"),
            Self::Discarded {
                ack_record_count,
                reason,
            } => write!(
                f,
                "discarded Teltonika frame with {ack_record_count} records: {reason}"
            ),
        }
    }
}

impl Error for TeltonikaFrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parse(err) => Some(err),
            Self::Discarded { reason, .. } => Some(reason),
        }
    }
}

impl fmt::Display for DiscardReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FutureTimestamp(err) => write!(f, "{err}"),
        }
    }
}

impl Error for DiscardReason {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FutureTimestamp(err) => Some(err),
        }
    }
}

impl From<io::Error> for TeltonikaFrameError {
    fn from(err: io::Error) -> Self {
        Self::Parse(err)
    }
}
