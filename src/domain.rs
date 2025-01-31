use std::collections::BTreeMap;

use chrono::{prelude::*, TimeDelta};
use derive_more::derive::Display;

use crate::repository::Repository;

/// Holds IDs to all event instances, allowing lookup by time.
pub struct Timeline<R: Repository + ?Sized> {
    pub events: BTreeMap<DateTime<Utc>, R::EventInstanceId>,
}

// we manually implement Default because the derive macro is not smart enough to
// apply the correct bounds, instead flatly refusing to implement Default
impl<R: Repository + ?Sized> Default for Timeline<R> {
    fn default() -> Self {
        Self {
            events: BTreeMap::new(),
        }
    }
}

impl<R: Repository + ?Sized> Timeline<R> {
    pub fn new() -> Self {
        Self::default()
    }
}

// we manually implement Debug because the derive macro is not smart enough to
// apply the correct bounds, instead flatly refusing to implement Debug
impl<R: Repository + ?Sized> std::fmt::Debug for Timeline<R>
where
    R: std::fmt::Debug,
    R::EventInstanceId: std::fmt::Debug,
    R::EventBodyId: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Timeline ")?;
        f.debug_map().entries(self.events.iter()).finish()
    }
}

/// A single event instance.
#[derive(Debug)]
pub struct EventInstance<R: Repository + ?Sized> {
    pub time_span: TimeSpan,
    pub body: R::EventBodyId,
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
}

#[derive(Debug)]
pub struct EventBody {
    pub summary: String,
    pub description: String,
    // TODO add location, categories, etc.
}
