use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use ical::property::Property;

type Result<T> = std::result::Result<T, String>;

pub trait IcalType {
    const TYPE_NAME: &'static str;
    // When stable: type Output = Self;
    type Output;

    fn parse(property: Property) -> Result<Self::Output>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IcalDateTime {
    Naive(NaiveDateTime),
    Utc(DateTime<Utc>),
    Tz(DateTime<Tz>),
}

impl IcalType for IcalDateTime {
    const TYPE_NAME: &'static str = "DATE-TIME";
    type Output = Self;

    fn parse(property: Property) -> Result<Self::Output> {
        let value_string = property.value.unwrap_or_default();

        let value = value_string.as_str();
        let (date_time, is_utc) = match value.strip_suffix('Z') {
            Some(date_time) => (date_time, true),
            None => (value, false),
        };

        let date_time = match NaiveDateTime::parse_from_str(date_time, "%Y%m%dT%H%M%S") {
            Ok(date_time) => date_time,
            Err(_) => return Err(value_string), // TODO
        };

        let params = property.params.as_deref().unwrap_or_default();
        let tz_id = params
            .iter()
            .rfind(|(n, _)| n == "TZID")
            .and_then(|(_, v)| v.last());

        match (is_utc, tz_id) {
            (true, Some(_)) => Err(value_string), // TODO
            (false, Some(tz_id)) => {
                let tz = tz_id.parse::<Tz>().map_err(|_| value_string)?; // TODO
                Ok(Self::Tz(tz.from_local_datetime(&date_time).unwrap())) // TODO unwrap
            }
            (true, None) => Ok(Self::Utc(Utc.from_utc_datetime(&date_time))),
            (false, None) => Ok(Self::Naive(date_time)),
        }
    }
}

pub struct IcalInt;

impl IcalType for IcalInt {
    const TYPE_NAME: &'static str = "INT";
    type Output = i32;

    fn parse(property: Property) -> Result<Self::Output> {
        property
            .value
            .as_deref()
            .unwrap_or_default()
            .parse::<i32>()
            .map_err(|_| property.value.unwrap_or_default())
    }
}

pub struct IcalText;

impl IcalType for IcalText {
    const TYPE_NAME: &'static str = "TEXT";
    type Output = String;

    fn parse(property: Property) -> Result<Self::Output> {
        let value = property.value.unwrap_or_default();

        // We attempt to reuse the string buffer if there's no replacement to be done
        if let Some(idx) = value.find('\\') {
            // FIXME: This algorithm is stupid and won't work as expected for i.e. «\\\\;»
            //        It should also probably fail if an invalid escape sequence is used

            let mut clone = value[..idx].to_string();
            clone += &value[idx..]
                .replace("\\n", "\n")
                .replace("\\N", "\n")
                .replace("\\;", ";")
                .replace("\\,", ",")
                .replace("\\\\", "\\");

            Ok(clone)
        } else {
            Ok(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};

    macro_rules! p {
        ($name:literal $(;$prop:literal = $prop_value:literal)* : $value:literal) => {
            Property {
                name: ToString::to_string($name),
                params: Some(vec![$(
                    (ToString::to_string($prop), vec![ToString::to_string($prop_value)]),
                )*]),
                value: Some(ToString::to_string($value)),
            }
        };
    }

    #[test]
    fn parse_ical_date_time() {
        assert_eq!(
            IcalDateTime::parse(p!("": "20020110T123045")).unwrap(),
            IcalDateTime::Naive(NaiveDate::from_ymd(2002, 1, 10).and_hms(12, 30, 45)),
        );

        assert_eq!(
            IcalDateTime::parse(p!("": "20020110T123045Z")).unwrap(),
            IcalDateTime::Utc(Utc.ymd(2002, 1, 10).and_hms(12, 30, 45)),
        );

        use chrono_tz::Europe::Paris;

        assert_eq!(
            IcalDateTime::parse(p!(""; "TZID"="Europe/Paris": "20020110T123045")).unwrap(),
            IcalDateTime::Tz(Paris.ymd(2002, 1, 10).and_hms(12, 30, 45)),
        );
    }

    #[test]
    fn parse_ical_date_time_invalid() {
        assert!(matches!(
            IcalDateTime::parse(p!(""; "TZID"="Middle_Earth/Minas_Tirith": "20020110T123045")),
            Err(_),
        ));

        assert!(matches!(
            IcalDateTime::parse(p!(""; "TZID"="Europe/Paris": "20020110T123045Z")),
            Err(_),
        ));
    }
}
