#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;
extern crate rusqlite;
extern crate chrono;
extern crate itertools;

use std::io;
use std::sync::Mutex;

use itertools::Itertools;
use rocket::{Rocket, State};
use rocket::http::RawStr;
use rocket::request::{Form, FormDataError, FormError};
use rocket::response::NamedFile;
use rocket::response::Redirect;
use rusqlite::{Connection, NO_PARAMS};
use rusqlite::types::ToSql;
use std::collections::HashMap;
use chrono::prelude::Local;

type DbConn = Mutex<Connection>;

fn init_database(conn: &Connection) {
    conn.execute("CREATE TABLE vote_results (
                  id              INTEGER PRIMARY KEY autoincrement,
                  place           TEXT NOT NULL,
                  date            TEXT NOT NULL,
                  username        TEXT NOT NULL
                  )", NO_PARAMS)
        .expect("create entries table");

    conn.execute("CREATE UNIQUE INDEX u_idx ON vote_results (place, username, date)", NO_PARAMS).expect("");
}

#[derive(Debug)]
struct Vote {
    username: String,
    place: String,
}


#[get("/results")]
fn results(db_conn: State<DbConn>) -> String  {
    let votes = db_conn.lock()
        .expect("db connection lock")
        .prepare("SELECT username, place FROM vote_results") // where date = $1
        .unwrap()
        .query_map(NO_PARAMS, |row| Vote {
            username: row.get(0),
            place: row.get(1)
        }).unwrap()
        .map(|tv| tv.unwrap()).collect_vec();

    let mut frequency: HashMap<&str, u32> = HashMap::new();
    for word in &votes { // word is a &str
        *frequency.entry(word.place.as_str()).or_insert(0) += 1;
    }

    format!("{:?} \n\n\n {:?}", frequency, votes)
}

#[derive(Debug, FromForm)]
struct FormInput<'r> {
    username: &'r RawStr,
    markthalle: bool,
    burgerlich: bool,
}

#[post("/vote", data = "<vote_form>")]
fn vote(vote_form: Result<Form<FormInput>, FormError>, db_conn: State<DbConn>) -> Redirect {
    match vote_form {
        Ok(form) => {
            let mut places = Vec::new();

            if form.burgerlich {places.push("Burgerlich")};
            if form.markthalle {places.push("Markthalle")};

            places.iter().for_each(|place| {
                db_conn
                    .lock()
                    .expect("db connection lock")
                    .execute("INSERT OR IGNORE INTO vote_results (place, date, username) VALUES ($1, $2, $3)",
                             &[&place as &ToSql, &Local::today().naive_local() as &ToSql, &form.username.as_str() as &ToSql])
                    .expect("insertion failed");
            });
            Redirect::to("/results")
        }
        Err(FormDataError::Io(_)) => {
            format!("Form input was invalid UTF-8.");
            Redirect::to("/error")
        }
        Err(FormDataError::Malformed(f)) | Err(FormDataError::Parse(_, f)) => {
            format!("Invalid form input: {}", f);
            Redirect::to("/error")
        }
    }
}

//static files
//rocket::ignite().mount("/", StaticFiles::from("static"))

#[get("/")]
fn index() -> io::Result<NamedFile> {
    NamedFile::open("static/index.html")
}


fn rocket() -> Rocket {
    // Open a new in-memory SQLite database.
    let conn = Connection::open_in_memory().expect("in memory db");

    // Initialize the `entries` table in the in-memory database.
    init_database(&conn);

    // Have Rocket manage the database pool.
    rocket::ignite()
        .manage(Mutex::new(conn))
        .mount("/", routes![index, results, vote])
}

fn main() {
    rocket().launch();
}