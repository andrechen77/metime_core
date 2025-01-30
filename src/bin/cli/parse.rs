use std::sync::LazyLock;

use chrono::prelude::*;
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

pub fn parse_lenient_date_time(input: &str) -> Option<DateTime<Utc>> {
    // match on the regex to get required fields
    let Some(captures) = RE_TIME_SPAN.captures(&input) else {
        return None;
    };

    // parse components from matched fields
    let year: i32 = captures.name("year").map_or_else(
        || Utc::now().year(),
        |m| m.as_str().parse().expect("regex guarantees digits"),
    );
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
    let hour: u32;
    let minute: u32;
    let second: u32;
    if let Some(h) = captures.name("hour") {
        hour = h.as_str().parse().expect("regex guarantees digits");
        minute = captures
            .name("minute")
            .expect("regex guarantees hour and minute exist together")
            .as_str()
            .parse()
            .expect("regex guarantees digits");
        second = captures
            .name("sec")
            .map_or(0, |m| m.as_str().parse().expect("regex guarantees digits"));
    } else {
        hour = 0;
        minute = 0;
        second = 0;
    }

    // build a date-time, depending on how the offset was specified
    let date_time: DateTime<Utc> = if captures.name("offset").is_some() {
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
            let offset = FixedOffset::east_opt(offset).expect("parsed offset should be valid");
            if let Some(dt) = offset
                .with_ymd_and_hms(year, month, day, hour, minute, second)
                .earliest()
            {
                dt.with_timezone(&Utc)
            } else {
                return None;
            }
        } else {
            // offset is specified with "Z", so use UTC
            if let Some(dt) = Utc
                .with_ymd_and_hms(year, month, day, hour, minute, second)
                .single()
            {
                dt
            } else {
                return None;
            }
        }
    } else {
        // offset is not specified, so use local time
        if let Some(dt) = Local
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .earliest()
        {
            dt.with_timezone(&Utc)
        } else {
            return None;
        }
    };

    Some(date_time)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lenient_date_time() {
        // Test with full date and time with UTC offset
        let input = "2023-10-05T14:30:00Z";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 14, 30, 0).unwrap();
        assert_eq!(parse_lenient_date_time(input), Some(expected));

        // Test with full date and time with positive offset
        let input = "2023-10-05T14:30:00+02:00";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 12, 30, 0).unwrap();
        assert_eq!(parse_lenient_date_time(input), Some(expected));

        // Test with full date and time with negative offset
        let input = "2023-10-05T14:30:00-02:00";
        let expected = Utc.with_ymd_and_hms(2023, 10, 5, 16, 30, 0).unwrap();
        assert_eq!(parse_lenient_date_time(input), Some(expected));

        // Test with date only
        let input = "2023-10-05";
        let expected = Local
            .with_ymd_and_hms(2023, 10, 5, 0, 0, 0)
            .earliest()
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parse_lenient_date_time(input), Some(expected));

        // Test with date and time without offset
        let input = "2023-10-05T14:30:00";
        let expected = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parse_lenient_date_time(input), Some(expected));

        // Test with date and time without seconds
        let input = "2023-10-05T14:30";
        let expected = Local
            .with_ymd_and_hms(2023, 10, 5, 14, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parse_lenient_date_time(input), Some(expected));

        // Test with partial date (month and day only)
        let input = "10-05";
        let now = Utc::now(); // lol don't test this on new year's eve
        let expected = Local
            .with_ymd_and_hms(now.year(), 10, 5, 0, 0, 0)
            .earliest()
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parse_lenient_date_time(input), Some(expected));
    }
}
