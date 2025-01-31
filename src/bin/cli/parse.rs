use std::sync::LazyLock;

use chrono::prelude::*;
use metime_core::TimeSpan;
use regex::Regex;

const PATTERN_TIME_SPAN: &str = r#"(?x)
    (?:
        (?<year> \d{4} )
        -
    )?
    (?<month> \d{1,2} )
    -
    (?<day> \d{1,2} )
    (?:
        T
        (?<hour> \d{1,2} )
        :
        (?<minute> \d{2} )
        (?:
            :
            (?<sec> \d{2} )
        )?
        (?<offset>
            Z
            |(?<noffset> [+-]\d{1,2}:\d{2} )
        )?
    )?
$"#;
static RE_TIME_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(PATTERN_TIME_SPAN).expect("hard-coded regex should compile"));

struct LexedTimeSpan {
    year: Option<i32>,
    month: u32,
    day: u32,
    time: Option<LexedTime>,
}

struct LexedTime {
    hour: u32,
    minute: u32,
    second: u32,
    offset: LexedOffset,
}

enum LexedOffset {
    Utc,
    /// The time zone offset in seconds; positive values are in the Eastern
    /// hemisphere.
    FixedOffset(i32),
    LocalTime,
}

fn lex_lenient_time_span(input: &str) -> Option<LexedTimeSpan> {
    // match on the regex to get required fields
    let captures = RE_TIME_SPAN.captures(input)?;

    // lex components from matched fields
    let year: Option<i32> = captures
        .name("year")
        .map(|m| m.as_str().parse().expect("regex guarantees digits"));
    let month: u32 = captures
        .name("month")
        .expect("regex guarantees month")
        .as_str()
        .parse()
        .expect("regex guarantees digits");
    let day: u32 = captures
        .name("day")
        .expect("regex guarantees day")
        .as_str()
        .parse()
        .expect("regex guarantees digits");

    // lex the time from the string
    let lexed_time: Option<LexedTime> = if let Some(h) = captures.name("hour") {
        let hour: u32 = h.as_str().parse().expect("regex guarantees digits");
        let minute: u32 = captures
            .name("minute")
            .expect("regex guarantees hour and minute exist together")
            .as_str()
            .parse()
            .expect("regex guarantees digits");
        let second: u32 = captures
            .name("sec")
            .map_or(0, |m| m.as_str().parse().expect("regex guarantees digits"));

        let lexed_offset = if captures.name("offset").is_some() {
            if let Some(offset_str) = captures.name("noffset") {
                // offset is specified as "+HH:MM" or "-HH:MM"
                use chrono::format;
                let mut parsed = format::Parsed::new();
                let _ = format::parse(
                    &mut parsed,
                    offset_str.as_str(),
                    format::StrftimeItems::new("%:z"),
                );
                let offset = parsed.offset().expect("regex guarantees valid offset");
                LexedOffset::FixedOffset(offset)
            } else {
                // offset is specified with "Z", so use UTC
                LexedOffset::Utc
            }
        } else {
            // offset is not specified, so use local time
            LexedOffset::LocalTime
        };

        Some(LexedTime {
            hour,
            minute,
            second,
            offset: lexed_offset,
        })
    } else {
        None
    };

    Some(LexedTimeSpan {
        year,
        month,
        day,
        time: lexed_time,
    })
}

pub fn parse_lenient_time_span(input: &str) -> Option<TimeSpan> {
    // lex the input
    let lexed = lex_lenient_time_span(input)?;
    let LexedTimeSpan {
        year, month, day, ..
    } = lexed;
    let year = year.unwrap_or_else(|| Utc::now().year());

    if let Some(lexed_time) = lexed.time {
        let LexedTime {
            hour,
            minute,
            second,
            ..
        } = lexed_time;
        let dt = match lexed_time.offset {
            LexedOffset::Utc => Utc
                .with_ymd_and_hms(year, month, day, hour, minute, second)
                .unwrap(),
            LexedOffset::FixedOffset(offset) => {
                let tz = FixedOffset::east_opt(offset).expect("lexed offset should be valid");
                tz.with_ymd_and_hms(year, month, day, hour, minute, second)
                    .earliest()
                    .map(|dt| dt.with_timezone(&Utc))?
            }
            LexedOffset::LocalTime => Local
                .with_ymd_and_hms(year, month, day, hour, minute, second)
                .earliest()
                .map(|dt| dt.with_timezone(&Utc))?,
        };

        Some(TimeSpan::Instant(dt))
    } else {
        // no time specified, so use a naive day
        todo!("implement naive day in TimeSpan");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lenient_time_span_instant() {
        // full date and time with UTC offset
        let input = "2023-10-05T14:30:00Z";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 14, 30, 0).unwrap();
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );

        // full date and time with positive offset
        let input = "2023-10-05T14:30:00+02:00";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 12, 30, 0).unwrap();
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );

        // full date and time with negative offset
        let input = "2023-10-05T14:30:00-02:00";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 16, 30, 0).unwrap();
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );

        // full date and time without offset
        let input = "2023-10-05T14:30:00";
        let expected = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );

        // date and time without seconds
        let input = "2023-10-05T14:30";
        let expected = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            parse_lenient_time_span(input),
            Some(TimeSpan::Instant(expected))
        );

        // date and time without years
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
}
