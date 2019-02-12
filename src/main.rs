#![feature(proc_macro_hygiene, decl_macro)]

extern crate chrono;
extern crate itertools;
#[macro_use] extern crate rocket;
extern crate rocket_contrib;
extern crate rusqlite;
#[macro_use] extern crate serde_derive;
extern crate serde_json;

use std::collections::HashMap;
use std::io;
use std::sync::Mutex;

use chrono::prelude::Local;
use itertools::Itertools;
use rocket::{Rocket, State};
use rocket::http::RawStr;
use rocket::request::{Form, FormDataError, FormError};
use rocket::response::NamedFile;
use rocket::response::Redirect;
use rocket_contrib::templates::Template;
use rusqlite::{Connection, NO_PARAMS};
use rusqlite::types::ToSql;

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

#[derive(Serialize, Deserialize, Default, Debug)]
struct Vote {
    username: String,
    place: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct TemplateContext<'a> {
    frequency: &'a str,
    votes: &'a str,
    // This key tells handlebars which template is the parent.
    parent: &'a str,
}


#[get("/results")]
fn results(db_conn: State<DbConn>) -> Template {
    let votes: Vec<Vote> = db_conn.lock()
        .expect("db connection lock")
        .prepare("SELECT username, place FROM vote_results") // where date = $1
        .unwrap()
        .query_map(NO_PARAMS, |row| {
            let username: String = row.get(0);
            let place: String = row.get(1);

            let vote = Vote {
                username,
                place
            };

            vote
        }).unwrap()
        .map(|tv| tv.unwrap()).collect_vec();

    let mut frequency: HashMap<&str, u32> = HashMap::new();
    for word in &votes {
        *frequency.entry(word.place.as_str()).or_insert(0) += 1;
    }

    Template::render("results", &TemplateContext {
        frequency: format!("{:#?}", frequency).as_str(),
        votes: format!("{:#?}", votes).as_str(),
        parent: "layout",
    })
}

#[derive(Debug, FromForm)]
struct FormInput<'r> {
    username: &'r RawStr,
    markthalle: bool,
    burgerlich: bool,
}

#[get("/error?<reason>")]
fn error(reason: String) -> Template {
    let problem = match reason.as_str() {
        "no_username" => "You didn't enter username!",
        "non_ascii" => "You entered non utf-8 character",
        _ => "Oops. Something went wrong."
    }.to_string();

    let mut map = std::collections::HashMap::new();
    map.insert("problem", problem);
    Template::render("error", &map)
}

#[post("/vote", data = "<vote_form>")]
fn vote(vote_form: Result<Form<FormInput>, FormError>, db_conn: State<DbConn>) -> Redirect {
    match vote_form {
        Ok(form) => {
            if form.username.is_empty() {
                return Redirect::to("/error?reason=no_username")
            }

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
            Redirect::to("/error?reason=non_ascii")
        }
        Err(FormDataError::Malformed(f)) | Err(FormDataError::Parse(_, f)) => {
            format!("Invalid form input: {}", f);
            Redirect::to("/error?reason=invalid_form")
        }
    }
}

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
        .mount("/", routes![index, results, vote, error])
        .attach(Template::fairing())
}

fn main() {
    rocket().launch();
}