use chrono::{Datelike, Timelike};
use curl::easy::Easy;
use pgx::*;
use pgx_named_columns::*;
use pipe::PipeReader;
use postgres_ical_parser::types::IcalDateTime;
use postgres_ical_parser::{CalendarParseError, Event};
use std::io::{BufRead, BufReader, Cursor, Write};
use std::thread::JoinHandle;
use time::{PrimitiveDateTime, UtcOffset};

pg_module_magic!();

/// [`curl`] is used instead of a Rustier alternative to make [`postgres_ical`] as lightweight as
/// possible
fn curl_get(url: &str) -> (PipeReader, JoinHandle<()>) {
    let (reader, mut writer) = pipe::pipe_buffered();

    let mut easy = Easy::new();
    easy.url(url).unwrap();

    let handle = std::thread::spawn(move || {
        let mut transfer = easy.transfer();
        transfer
            .write_function(move |data| {
                writer.write_all(data).unwrap();
                Ok(data.len())
            })
            .unwrap();

        transfer.perform().unwrap();
        std::mem::drop(transfer);
    });

    (reader, handle)
}

fn to_time(d: impl Datelike + Timelike) -> PrimitiveDateTime {
    use time::Month::*;
    use time::*;

    let month = match d.month() {
        1 => January,
        2 => February,
        3 => March,
        4 => April,
        5 => May,
        6 => June,
        7 => July,
        8 => August,
        9 => September,
        10 => October,
        11 => November,
        12 => December,
        _ => unreachable!(),
    };

    PrimitiveDateTime::new(
        Date::from_calendar_date(d.year(), month, d.day() as u8).unwrap(),
        Time::from_hms(d.hour() as u8, d.minute() as u8, d.second() as u8).unwrap(),
    )
}

fn serialize_datetime(date: IcalDateTime) -> (Option<TimestampWithTimeZone>, Option<Timestamp>) {
    match date {
        IcalDateTime::Naive(naive) => (None, Some(Timestamp::new(to_time(naive)))),
        IcalDateTime::Utc(utc) => (
            Some(TimestampWithTimeZone::new(to_time(utc), UtcOffset::UTC)),
            None,
        ),
        IcalDateTime::Tz(tz) => {
            use chrono::Offset;
            let offset = tz.offset().fix().local_minus_utc();
            let offset = UtcOffset::from_whole_seconds(offset).unwrap();
            (Some(TimestampWithTimeZone::new(to_time(tz), offset)), None)
        }
    }
}

/// TODO
#[deprecated]
type Interval = i16;

#[derive(PostgresEnum)]
pub enum ComponentType {
    VCALENDAR,
    VEVENT,
    VTODO,
    VJOURNAL,
    VFREEBUSY,
    VTIMEZONE,
    VALARM,
}

#[derive(PostgresEnum)]
pub enum Class {
    PUBLIC,
    PRIVATE,
    CONFIDENTIAL,
}

#[derive(PostgresEnum)]
pub enum Status {
    TENTATIVE,
    CONFIRMED,
    CANCELLED,
    NEEDSACTION,
    COMPLETED,
    INPROCESS,
    DRAFT,
    FINAL,
}

/// Represents a row returned by [pg_ical] or [pg_ical_curl]
pub struct Component {
    pub component_type: ComponentType,
    pub attachment: Option<String>,
    pub categories: Vec<String>,
    pub class: Option<Class>,
    pub comment: Vec<String>,
    pub completed: Option<TimestampWithTimeZone>,
    pub completed_naive: Option<Timestamp>,
    pub created: Option<TimestampWithTimeZone>,
    pub created_naive: Option<Timestamp>,
    pub description: Option<String>,
    pub dt_stamp: Option<TimestampWithTimeZone>,
    pub dt_stamp_naive: Option<Timestamp>,
    pub dt_start: Option<TimestampWithTimeZone>,
    pub dt_start_naive: Option<Timestamp>,
    pub dt_end: Option<TimestampWithTimeZone>,
    pub dt_end_naive: Option<Timestamp>,
    pub due: Option<TimestampWithTimeZone>,
    pub due_naive: Option<Timestamp>,
    pub duration: Option<Interval>,
    pub geo_lat: Option<f32>,
    pub geo_lng: Option<f32>,
    pub last_modified: Option<TimestampWithTimeZone>,
    pub last_modified_naive: Option<Timestamp>,
    pub location: Option<String>,
    pub percent_complete: Option<i32>,
    pub priority: Option<i32>,
    pub resources: Vec<String>,
    pub status: Option<Status>,
    pub sequence: i32,
    pub summary: Option<String>,
    pub uid: String,
}

fn convert_component(res: Result<Event, CalendarParseError>) -> Component {
    let event = res.unwrap();

    let (created, created_naive) = event.created.map(serialize_datetime).unwrap_or_default();
    let (dt_stamp, dt_stamp_naive) = event.dt_stamp.map(serialize_datetime).unwrap_or_default();
    let (dt_start, dt_start_naive) = serialize_datetime(event.dt_start);
    let (dt_end, dt_end_naive) = event.dt_end.map(serialize_datetime).unwrap_or_default();
    let (last_modified, last_modified_naive) = event
        .last_modified
        .map(serialize_datetime)
        .unwrap_or_default();

    Component {
        component_type: ComponentType::VEVENT,
        attachment: None,       // TODO
        categories: Vec::new(), // TODO
        class: None,            // TODO
        comment: Vec::new(),    // TODO
        completed: None,        // TODO
        completed_naive: None,  // TODO
        created,
        created_naive,
        description: event.description,
        dt_stamp,
        dt_stamp_naive,
        dt_start,
        dt_start_naive,
        dt_end,
        dt_end_naive,
        due: None,       // TODO
        due_naive: None, // TODO
        duration: None,  // TODO
        geo_lat: None,   // TODO
        geo_lng: None,   // TODO
        last_modified,
        last_modified_naive,
        location: event.location,
        percent_complete: None, // TODO
        priority: None,         // TODO
        resources: Vec::new(),  // TODO
        status: None,           // TODO
        sequence: event.sequence,
        summary: event.summary,
        uid: event.uid,
    }
}

fn pg_ical_internal(calendar: impl BufRead) -> impl Iterator<Item = Component> {
    let parser = postgres_ical_parser::EventsReader::new(calendar);
    parser.map(convert_component)
}

/// Load an [`ical`][ical] file from an in-memory text representation
///
/// The number of columns may increase at any moment without it being considered a breaking change.
/// For forward-compatibility, when consuming this function's output, always do an explicit select.
/// Column deletion or altering is — however, and obviously — considered breaking.
///
/// [ical]: https://datatracker.ietf.org/doc/html/rfc5545
#[pg_extern_columns("src/lib.rs")]
pub fn pg_ical(calendar: String) -> impl Iterator<Item = Component> {
    pg_ical_internal(BufReader::new(Cursor::new(calendar.into_bytes())))
}

/// Load an [`ical`][ical] file from an URL, making a [curl] request in the process
///
/// The number of columns may increase at any moment without it being considered a breaking change.
/// For forward-compatibility, when consuming this function's output, always do an explicit select.
/// Column deletion or altering is — however, and obviously — considered breaking.
///
/// [ical]: https://datatracker.ietf.org/doc/html/rfc5545
#[pg_extern_columns("src/lib.rs")]
pub fn pg_ical_curl(url: &str) -> impl Iterator<Item = Component> {
    let (reader, handle) = curl_get(url);
    let mut handle = Some(handle);

    pg_ical_internal(reader).chain(std::iter::from_fn(move || {
        handle.take().unwrap().join().unwrap();
        None
    }))
}
