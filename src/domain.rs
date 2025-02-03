use std::collections::BTreeMap;

use chrono::{prelude::*, TimeDelta};
use derive_more::derive::Display;

/// Holds IDs to all event instances, allowing lookup by time.
#[derive(Debug)]
pub struct Timeline<EventInstanceId> {
    pub events: BTreeMap<DateTime<Utc>, EventInstanceId>,
}

impl<EventInstanceId> Default for Timeline<EventInstanceId> {
    fn default() -> Self {
        Self {
            events: BTreeMap::new(),
        }
    }
}

impl<EventInstanceId> Timeline<EventInstanceId> {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A single event instance.
#[derive(Debug)]
pub struct EventInstance<EventBodyId> {
    pub time_span: TimeSpan,
    pub body: EventBodyId,
}

/// A set of continuous points in time describing the times at which an event is
/// occuring. If the span is not instantaneous, the start endpoint is considered
/// included and the end endpoint is considered excluded (half-open interval).
#[derive(Debug, Display, PartialEq, Eq)]
pub enum TimeSpan {
    #[display("[{}]", _0.format("%c"))]
    Instant(DateTime<Utc>),
    #[display("[{} -- {}m]", start.format("%c"), duration.num_minutes())]
    Interval {
        start: DateTime<Utc>,
        duration: TimeDelta,
    },
    // TODO add dates and date intervals (without times)
}

impl TimeSpan {
    /// Returns the earliest point of the time span.
    pub fn earliest(&self) -> DateTime<Utc> {
        match self {
            TimeSpan::Instant(time) => *time,
            TimeSpan::Interval { start, .. } => *start,
        }
    }

    /// Returns the latest point of the time span. Since time spans are
    /// technically half-open intervals, this point is not actually included
    /// in the span.
    pub fn latest(&self) -> DateTime<Utc> {
        match self {
            TimeSpan::Instant(time) => *time,
            TimeSpan::Interval { start, duration } => *start + *duration,
        }
    }
}

#[derive(Debug)]
pub struct EventBody {
    pub summary: String,
    pub description: String,
    // TODO add location, categories, etc.
}
