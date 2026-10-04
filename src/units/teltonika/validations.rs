use std::{
    error::Error,
    fmt, io,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_FUTURE_TIMESTAMP_DRIFT_MS: u64 = 5 * 60 * 1000;

#[derive(Debug)]
pub enum TimestampValidationError {
    Clock(io::Error),
    TooFarInFuture(FutureTimestampError),
}

#[derive(Debug)]
pub struct FutureTimestampError {
    pub timestamp_ms: u64,
    pub now_ms: u64,
    pub max_future_drift_ms: u64,
}

pub fn validate_timestamp(timestamp_ms: u64) -> Result<(), TimestampValidationError> {
    let now_ms = current_timestamp_ms().map_err(TimestampValidationError::Clock)?;
    validate_timestamp_at(timestamp_ms, now_ms)
}

fn current_timestamp_ms() -> io::Result<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis(),
    )
    .map_err(io::Error::other)
}

fn validate_timestamp_at(timestamp_ms: u64, now_ms: u64) -> Result<(), TimestampValidationError> {
    if timestamp_ms > now_ms.saturating_add(MAX_FUTURE_TIMESTAMP_DRIFT_MS) {
        return Err(TimestampValidationError::TooFarInFuture(
            FutureTimestampError {
                timestamp_ms,
                now_ms,
                max_future_drift_ms: MAX_FUTURE_TIMESTAMP_DRIFT_MS,
            },
        ));
    }

    Ok(())
}

impl fmt::Display for TimestampValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(err) => write!(f, "failed to read system time: {err}"),
            Self::TooFarInFuture(err) => write!(f, "{err}"),
        }
    }
}

impl Error for TimestampValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(err) => Some(err),
            Self::TooFarInFuture(err) => Some(err),
        }
    }
}

impl fmt::Display for FutureTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "timestamp {} is too far in the future; server time is {} and max drift is {} ms",
            self.timestamp_ms, self.now_ms, self.max_future_drift_ms
        )
    }
}

impl Error for FutureTimestampError {}

#[cfg(test)]
mod tests {
    use super::{MAX_FUTURE_TIMESTAMP_DRIFT_MS, TimestampValidationError, validate_timestamp_at};

    #[test]
    fn accepts_timestamp_within_future_drift_window() {
        let now_ms = 1_700_000_000_000;
        let timestamp_ms = now_ms + MAX_FUTURE_TIMESTAMP_DRIFT_MS;

        assert!(validate_timestamp_at(timestamp_ms, now_ms).is_ok());
    }

    #[test]
    fn rejects_timestamp_beyond_future_drift_window() {
        let now_ms = 1_700_000_000_000;
        let timestamp_ms = now_ms + MAX_FUTURE_TIMESTAMP_DRIFT_MS + 1;

        let err = validate_timestamp_at(timestamp_ms, now_ms).unwrap_err();

        assert!(matches!(err, TimestampValidationError::TooFarInFuture(_)));
    }
}
