use chrono::{Duration, Utc};
use db::entities::CalendarEventDraft;
use icalendar::{Calendar, CalendarDateTime, Component, DatePerhapsTime, EventLike};

const MAX_EVENTS_PER_FEED: usize = 2_500;
const MAX_OCCURRENCES_PER_EVENT: u16 = 500;

pub struct CalendarSnapshot {
    pub name: String,
    pub events: Vec<CalendarEventDraft>,
}

/// Parses an RFC 5545 calendar into a bounded local event snapshot.
pub fn parse_calendar(bytes: &[u8]) -> Result<CalendarSnapshot, String> {
    let source =
        std::str::from_utf8(bytes).map_err(|_| "calendar file must be UTF-8 encoded".to_owned())?;
    let calendar: Calendar = source
        .parse()
        .map_err(|error| format!("calendar file could not be parsed: {error}"))?;
    let name = calendar
        .get_name()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Calendar")
        .chars()
        .take(120)
        .collect();
    let range_start = (Utc::now() - Duration::days(366)).with_timezone(&icalendar::rrule::Tz::UTC);
    let range_end =
        (Utc::now() + Duration::days(366 * 3)).with_timezone(&icalendar::rrule::Tz::UTC);
    let mut events = Vec::new();

    for (event_index, event) in calendar.events().enumerate() {
        let Some(start) = event.get_start() else {
            continue;
        };
        let all_day = matches!(start, DatePerhapsTime::Date(_));
        let duration = event_duration(&start, event.get_end().as_ref());
        let title = clipped(event.get_summary().unwrap_or("Untitled event"), 500);
        let description = clipped(event.get_description().unwrap_or(""), 10_000);
        let location = clipped(event.get_location().unwrap_or(""), 500);
        let url = clipped(event.get_url().unwrap_or(""), 2_048);
        let uid = event
            .get_uid()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map_or_else(
                || format!("event-{event_index}"),
                |value| clipped(value, 2_048),
            );
        let recurrence = event
            .get_recurrence()
            .map_err(|error| format!("calendar recurrence could not be parsed: {error}"))?;
        let dates = recurrence
            .after(range_start)
            .before(range_end)
            .all(MAX_OCCURRENCES_PER_EVENT)
            .dates;

        for occurrence in dates {
            let (start_at, end_at) = if all_day {
                let date = occurrence.date_naive();
                let end = duration.map(|value| date + value);
                (
                    date.format("%Y-%m-%d").to_string(),
                    end.map(|value| value.format("%Y-%m-%d").to_string()),
                )
            } else {
                let start_utc = occurrence.with_timezone(&Utc);
                let end = duration.map(|value| start_utc + value);
                (start_utc.to_rfc3339(), end.map(|value| value.to_rfc3339()))
            };
            events.push(CalendarEventDraft {
                external_id: uid.clone(),
                title: title.clone(),
                description: description.clone(),
                location: location.clone(),
                url: url.clone(),
                start_at,
                end_at,
                all_day,
            });
            if events.len() >= MAX_EVENTS_PER_FEED {
                return Ok(CalendarSnapshot { name, events });
            }
        }
    }

    Ok(CalendarSnapshot { name, events })
}

fn event_duration(start: &DatePerhapsTime, end: Option<&DatePerhapsTime>) -> Option<Duration> {
    match (start, end) {
        (DatePerhapsTime::Date(start), Some(DatePerhapsTime::Date(end))) => Some(*end - *start),
        (DatePerhapsTime::DateTime(start), Some(DatePerhapsTime::DateTime(end))) => {
            let start = calendar_datetime_utc(start)?;
            let end = calendar_datetime_utc(end)?;
            Some(end - start)
        }
        _ => None,
    }
}

fn calendar_datetime_utc(value: &CalendarDateTime) -> Option<chrono::DateTime<Utc>> {
    match value {
        CalendarDateTime::Floating(value) => Some(value.and_utc()),
        CalendarDateTime::Utc(value) => Some(*value),
        CalendarDateTime::WithTimezone { .. } => value.try_into_utc(),
    }
}

fn clipped(value: &str, max: usize) -> String {
    value.trim().chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calendar_name_and_recurring_events() {
        let start = Utc::now().format("%Y%m%dT090000Z");
        let source = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-CALNAME:Work\r\nBEGIN:VEVENT\r\nUID:standup\r\nDTSTART:{start}\r\nDTEND:{}\r\nRRULE:FREQ=DAILY;COUNT=2\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            (Utc::now() + Duration::hours(1)).format("%Y%m%dT%H%M%SZ")
        );
        let snapshot = parse_calendar(source.as_bytes()).expect("calendar parses");
        assert_eq!(snapshot.name, "Work");
        assert_eq!(snapshot.events.len(), 2);
        assert_eq!(snapshot.events[0].title, "Standup");
    }
}
