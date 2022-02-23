# `postgres-ical`

`postgres-ical` is a PostgreSQL extension that adds features related to parsing [RFC-5545 « iCalendar »](https://datatracker.ietf.org/doc/html/rfc5545) data from within a PostgreSQL database.

## Why ?

_iCalendar_ files are nothing more than a big table of « components » with a lot of properties. That's what relational databases handle every day.

The format is specifically designed not as a way to store calendar data, but as a way to transfer calendar data from a piece of software to another. Quite a few online calendar software programs allow their users to export a live version of their calendars as an _iCalendar_ URL. There are many situations in which being able to run SQL queries on such a file maybe be useful. Also, importing an _iCalendar_ file in an SQL database can be used as an easy caching system.

> Can't we simply have a client daemon do this syncing between the database and the remote source ?

Yes, definitely. And sometimes, installing PostgreSQL extensions is simply not possible. However, here are a few advantages of having the querying and parsing done from within the database :
  1. [Separation of concerns](https://en.wikipedia.org/wiki/Separation_of_concerns): Whatever it is you're actually doing with the calendar data, it's probably not the business of your application to keep the table of events in sync. This whole system is just a kind of one-way replication, which is usually done by the database and not by database clients.
  2. Atomicity and correctness: more often than not, the queried calendar data would be stored in a `materialized view`. Even if you could store your data in a normal table and always try to use a transaction to update it, nothing prevents you fundamentally, from doing "illegal" operations on the table, such as updates or insertions of data that is not present in the actual _iCalendar_ source.
  3. Unique source of truth: since the calendar data is actually a replication of data from elsewhere, by using a `materialized view` that directly queries the source on refresh, it is clear to anyone reading your database structure that the data is a copy from elsewhere. Additionally, the queried data would not appear in a database dump, which just makes sense since it's replicated.
  4. Simplicity ?: Depending on your use-case, doing part of the work inside the database might just be simpler than having to set up multiple clients, multiple systemd services, etc.

## Building

_To be documented..._

## Usage

After installing the extension, you can use the 2 following functions :

```sql
select * from pg_ical('BEGIN:VCALENDAR...');
select * from pg_ical_curl('https://example.com/calendar.ical');
```

The columns that are returned are documented on the Rustdoc, by the structure called `Component`. You can build the Rustdoc using `cargo doc --no-deps --open`.

Regarding compatibility and versioning, I don't consider column additions to be breaking changes, but alterations and deletions obviously are. You should ideally use precise `select` statements in order not to have surprises.

## Tech stack

The extension is made in Rust, with [the `pgx` library](https://github.com/zombodb/pgx) doing the rotten job of handling FFI. General _iCalendar_ parsing is done by the [`ical`](https://github.com/Peltoche/ical-rs) crate, while the actual meaning of properties is inferred by a local crate (`/postgres-ical-parser`), that will be published independently one day.

## License

The project doesn't have I license yet, and is thus considered "all rights reserved". I probably won't sue you if use it, but you technically don't have permission for the moment. I'm looking for a license similar to the AGPL, that would require you to distribute the source code to any remote users who ask for it, but in a way that doesn't infect your SQL code that uses the library functions (similar to the LGPL). An "LAGPL", so to say. Suggestions are welcome.
