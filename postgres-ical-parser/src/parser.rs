//! Type-safe ical event representation

use super::types::{IcalDateTime, IcalInt, IcalText, IcalType};
use ical::parser::ParserError;
use ical::property::{Property, PropertyError};
use ical::PropertyParser;
use std::io::BufRead;

pub struct Event {
    pub created: Option<IcalDateTime>,

    pub description: Option<String>,

    pub dt_stamp: Option<IcalDateTime>,

    pub dt_start: IcalDateTime,

    pub dt_end: Option<IcalDateTime>,

    pub last_modified: Option<IcalDateTime>,

    pub location: Option<String>,

    pub sequence: i32,

    pub summary: Option<String>,

    pub uid: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CalendarParseError {
    #[error("missing property {0}")]
    MissingProperty(&'static str),

    #[error("invalid property value {property}:{found:?}, expected {expected}")]
    InvalidPropertyValue {
        property: &'static str,
        found: String,
        expected: &'static str,
    },

    #[error("unknown property {0}")]
    UnknownProperty(String),

    #[error("internal ical parser error: {0}")]
    ParserError(#[from] ParserError),
}

fn ical_parse<T: IcalType>(
    property_name: &'static str,
    property: Property,
) -> Result<T::Output, CalendarParseError> {
    T::parse(property).map_err(|value| CalendarParseError::InvalidPropertyValue {
        property: property_name,
        found: value,
        expected: T::TYPE_NAME,
    })
}

macro_rules! event_from_properties {
    {
        for $property:ident in $properties:expr;
        $($name:literal $(! $($dummy:literal)*)? => $var:ident: $ical_type:ty $(= $default:expr)?,)*
    } => {
        $(let mut $var = event_from_properties!(@i $name; $property; $ical_type $(= $default)?);)*

        for $property in $properties {
            let $property = $property.map_err(ParserError::PropertyError)?;

            match $property.name.to_ascii_uppercase().as_str() {
                $($name => $var = event_from_properties!(@s $name; $property; $ical_type $(= $default)?),)*
                _ => (), // Unknown property
            }
        }

        Ok(Self {
            $($var $(: $var.ok_or(CalendarParseError::MissingProperty(event_from_properties!(@t $name @ $($dummy)*)))?)?,)*
        })
    };
    (@i $name:literal; $property:ident; $ical_type:ty = $default:expr) => { $default };
    (@s $name:literal; $property:ident; $ical_type:ty = $default:expr) => { ical_parse::<$ical_type>($name, $property)? };
    (@i $name:literal; $property:ident; $ical_type:ty) => { None };
    (@s $name:literal; $property:ident; $ical_type:ty) => { Some(ical_parse::<$ical_type>($name, $property)?) };
    (@t $lit:literal @ $($tt:tt)*) => { $lit };
}

impl Event {
    fn from_properties(
        properties: impl Iterator<Item = Result<Property, PropertyError>>,
    ) -> Result<Self, CalendarParseError> {
        event_from_properties! {
            for property in properties;
            "CREATED" => created: IcalDateTime,
            "DESCRIPTION" => description: IcalText,
            "DTSTART"! => dt_start: IcalDateTime,
            "DTSTAMP" => dt_stamp: IcalDateTime,
            "DTEND" => dt_end: IcalDateTime,
            "LAST-MODIFIED" => last_modified: IcalDateTime,
            "LOCATION" => location: IcalText,
            "SEQUENCE" => sequence: IcalInt = 0,
            "SUMMARY" => summary: IcalText,
            "UID"! => uid: IcalText,
        }
    }
}

pub struct EventsReader<R: BufRead> {
    raw_reader: PropertyParser<R>,
}

impl<R: BufRead> EventsReader<R> {
    pub fn new(buf_read: R) -> Self {
        let raw_reader = PropertyParser::new(ical::LineReader::new(buf_read));

        Self { raw_reader }
    }
}

impl<R: BufRead> Iterator for EventsReader<R> {
    type Item = Result<Event, CalendarParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            break match self.raw_reader.next() {
                None => None,
                Some(Err(err)) => Some(Err(CalendarParseError::ParserError(err.into()))),
                Some(Ok(mut property)) => {
                    property.name.make_ascii_uppercase();
                    match property.name.as_str() {
                        "BEGIN" => match property.value.as_deref() {
                            None => Some(Err(ParserError::InvalidComponent.into())),
                            Some("VEVENT") => {
                                Some(Event::from_properties(
                                    (&mut self.raw_reader).take_while(
                                        |property| !matches!(property, Ok(p) if p.name.as_str() == "END" && p.value.as_deref() == Some("VEVENT"))
                                    )
                                ))
                            }
                            Some("VCALENDAR") => continue,
                            Some(_other) => {
                                // TODO
                                continue;
                            }
                        },
                        _ => {
                            // TODO
                            continue
                        }
                    }
                }
            };
        }
    }
}
