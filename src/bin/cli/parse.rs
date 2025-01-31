use chrono::prelude::*;
use metime_core::TimeSpan;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum LexedTimeSpan {
    // TODO add a start-duration variant of intervals
    Instant(LexedInstant),
    InstantIntervalStartEnd {
        start: LexedInstant,
        end: LexedInstant,
    },
    DateIntervalStartDuration {
        start: LexedDate,
        /// The duration of the event in days.
        duration_days: Option<u32>,
    },
    DateIntervalStartEnd {
        start: LexedDate,
        end: LexedDate,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct LexedInstant {
    date: LexedDate,
    time: LexedTime,
    offset: LexedOffset,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct LexedDate {
    year: Option<i32>,
    month: u32,
    day: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct LexedTime {
    hour: u32,
    min: u32,
    sec: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum LexedOffset {
    Utc,
    /// The time zone offset in seconds; positive values are in the Eastern
    /// hemisphere.
    FixedOffset(i32),
    LocalTime,
}

peg::parser! {
    grammar time_span_parser() for str {
        use std::ops::RangeInclusive;

        rule decimal_int(digits: RangeInclusive<usize>) -> u32 =
            num:$(['0'..='9']*<{*digits.start()},{*digits.end()}>) { num.parse().unwrap() }

        rule utc_offset() = "Z"

        rule sign() -> i32 = "+" { 1 } / "-" { -1 }

        /// Parses +HH:MM or -HH:MM as a signed number of seconds.
        rule fixed_offset() -> i32 = s:sign() hours:decimal_int(1..=2) ":" mins:decimal_int(2..=2) {
            s * (hours as i32 * 3600 + mins as i32 * 60)
        }

        rule offset() -> LexedOffset = utc_offset() { LexedOffset::Utc } / o:fixed_offset() { LexedOffset::FixedOffset(o) } / { LexedOffset::LocalTime }

        rule time() -> LexedTime = h:decimal_int(1..=2) ":" m:decimal_int(2..=2) s:(":" s:decimal_int(2..=2) { s })? {
            LexedTime { hour: h, min: m, sec: s.unwrap_or(0) }
        }

        rule date() -> LexedDate = y:(y:decimal_int(4..=4) "-" { y })? m:decimal_int(1..=2) "-" d:decimal_int(1..=2) {
            LexedDate { year: y.map(|y| y as i32), month: m, day: d }
        }

        rule instant() -> LexedInstant = d:date() "T" t:time() o:offset() {
            LexedInstant { date: d, time: t, offset: o }
        }

        pub rule time_span() -> LexedTimeSpan = (start:instant() "/" end:instant() {
            LexedTimeSpan::InstantIntervalStartEnd { start, end }
        }) / (start:instant() "/" end_time:time() {
            LexedTimeSpan::InstantIntervalStartEnd { start, end: LexedInstant { time: end_time, ..start } }
        }) / (start:instant() {
            LexedTimeSpan::Instant(start)
        }) / (start:date() "/" end:date() {
            LexedTimeSpan::DateIntervalStartEnd { start, end }
        }) / (start:date() "/" duration:decimal_int(usize::MAX..=usize::MAX) {
            LexedTimeSpan::DateIntervalStartDuration { start, duration_days: Some(duration) }
        }) / (start:date() {
            LexedTimeSpan::DateIntervalStartDuration { start, duration_days: None }
        })
    }
}

pub fn parse_lenient_time_span(input: &str) -> Option<TimeSpan> {
    // lex the input
    let lexed = time_span_parser::time_span(input).ok()?;

    fn parse_date(date: LexedDate) -> Option<NaiveDate> {
        let LexedDate { year, month, day } = date;
        let year = year.unwrap_or_else(|| Utc::now().year());
        NaiveDate::from_ymd_opt(year, month, day)
    }

    fn parse_instant(instant: LexedInstant) -> Option<DateTime<Utc>> {
        let LexedInstant { date, time, offset } = instant;
        let LexedTime { hour, min, sec } = time;

        let naive_date = parse_date(date)?;
        let naive_dt = naive_date.and_hms_opt(hour, min, sec)?;

        let dt = match offset {
            LexedOffset::Utc => naive_dt.and_local_timezone(Utc).unwrap(),
            LexedOffset::FixedOffset(offset) => naive_dt
                .and_local_timezone(FixedOffset::east_opt(offset)?)
                .earliest()
                .map(|dt| dt.with_timezone(&Utc))?,
            LexedOffset::LocalTime => naive_dt
                .and_local_timezone(Local)
                .earliest()
                .map(|dt| dt.with_timezone(&Utc))?,
        };
        Some(dt)
    }

    match lexed {
        LexedTimeSpan::Instant(instant) => Some(TimeSpan::Instant(parse_instant(instant)?)),
        LexedTimeSpan::InstantIntervalStartEnd { start, end } => {
            let start = parse_instant(start)?;
            let end = parse_instant(end)?;
            let duration = end - start;
            Some(TimeSpan::Interval { start, duration })
        }
        LexedTimeSpan::DateIntervalStartDuration { .. } => {
            todo!("implement date timespans")
        }
        LexedTimeSpan::DateIntervalStartEnd { .. } => {
            todo!("implement date timespans")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO fuzzing test

    #[test]
    fn parse_date_and_time_with_utc_offset() {
        let input = "2023-10-05T14:30:00Z";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 14, 30, 0).unwrap();
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );
    }

    #[test]
    fn parse_date_and_time_with_positive_offset() {
        let input = "2023-10-05T14:30:00+02:00";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 12, 30, 0).unwrap();
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );
    }

    #[test]
    fn parse_date_and_time_with_negative_offset() {
        let input = "2023-10-05T14:30:00-02:00";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 16, 30, 0).unwrap();
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );
    }

    #[test]
    fn parse_date_and_time_without_offset() {
        let input = "2023-10-05T14:30:00";
        let expected = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );
    }

    #[test]
    fn parse_date_and_time_without_seconds() {
        let input = "2023-10-05T14:30";
        let expected = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );
    }

    #[test]
    fn parse_date_and_time_without_seconds_with_offset() {
        let input = "2023-10-05T14:30+02:00";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 12, 30, 0).unwrap();
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );
    }

    #[test]
    fn parse_date_and_time_without_years() {
        let input = "10-05T14:30:00";
        let now = Utc::now(); // lol don't test this on new year's eve
        let expected = Local
            .with_ymd_and_hms(now.year(), 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );
    }

    #[test]
    fn parse_instant_interval_with_utc_offset() {
        let input = "2023-10-05T14:30:00Z/2023-10-05T16:30:00Z";
        let start = Utc.with_ymd_and_hms(2023, 10, 5, 14, 30, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2023, 10, 5, 16, 30, 0).unwrap();
        let duration = end - start;
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Interval { start, duration })
        );
    }

    #[test]
    fn parse_instant_interval_with_positive_offset() {
        let input = "2023-10-05T14:30:00+02:00/2023-10-05T16:30:00+02:00";
        let start = Utc.with_ymd_and_hms(2023, 10, 5, 12, 30, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2023, 10, 5, 14, 30, 0).unwrap();
        let duration = end - start;
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Interval { start, duration })
        );
    }

    #[test]
    fn parse_instant_interval_with_negative_offset() {
        let input = "2023-10-05T14:30:00-02:00/2023-10-05T16:30:00-02:00";
        let start = Utc.with_ymd_and_hms(2023, 10, 5, 16, 30, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2023, 10, 5, 18, 30, 0).unwrap();
        let duration = end - start;
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Interval { start, duration })
        );
    }

    #[test]
    fn parse_instant_interval_without_offset() {
        let input = "2023-10-05T14:30:00/2023-10-05T16:30:00";
        let start = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        let end = Local
            .with_ymd_and_hms(2023, 10, 5, 16, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        let duration = end - start;
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Interval { start, duration })
        );
    }

    #[test]
    fn parse_instant_interval_without_seconds() {
        let input = "2023-10-05T14:30/2023-10-05T16:30";
        let start = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        let end = Local
            .with_ymd_and_hms(2023, 10, 5, 16, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        let duration = end - start;
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Interval { start, duration })
        );
    }

    #[test]
    fn parse_instant_interval_without_seconds_with_offset() {
        let input = "2023-10-05T14:30+02:00/2023-10-05T16:30+02:00";
        let start = Utc.with_ymd_and_hms(2023, 10, 5, 12, 30, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2023, 10, 5, 14, 30, 0).unwrap();
        let duration = end - start;
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Interval { start, duration })
        );
    }

    #[test]
    fn parse_instant_interval_omitting_end_date_and_offset() {
        let input = "2023-10-05T14:30:00/16:30:00";
        let start = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        let end = Local
            .with_ymd_and_hms(2023, 10, 5, 16, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        let duration = end - start;
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Interval { start, duration })
        );
    }
}
