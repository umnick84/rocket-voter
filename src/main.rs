#![feature(proc_macro_hygiene, decl_macro)]

extern crate chrono;
extern crate itertools;
#[macro_use] extern crate rocket;
extern crate rocket_contrib;
extern crate rusqlite;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
#[macro_use] extern crate lazy_static;

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::prelude::Local;
use itertools::Itertools;
use rocket::{Rocket, State};
use rocket::http::RawStr;
use rocket::request::{Form, FormDataError, FormError};
use rocket::response::Redirect;
use rocket_contrib::templates::Template;
use rusqlite::{Connection, NO_PARAMS};
use rusqlite::types::ToSql;

lazy_static! {
    static ref PLACES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("markthalle", "Markthalle");
        m.insert("burgerlich", "Burgerlich");
        m.insert("andronaco", "Andronaco");
        m.insert("hans_in_glueck", "Hans im Glück");
        m.insert("thai_food", "Thai-Food");
        m.insert("wildes_fraeulein", "Wildes Fräulein");
        m.insert("sala_thai", "Sala Thai");
        m.insert("galette_de_bretagne", "Galette de Bretagne");
        m.insert("mozzers", "Mozzer’s");
        m.insert("chie_tu_huang", "Chie-Tu Huang");
        m.insert("kartoffelkeller", "Kartoffelkeller");
        m.insert("o_ren_ishii", "O-Ren Ishii");
        m.insert("dos_amigos", "Dos Amigos");
        m
    };
}

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

#[derive(Serialize, Deserialize, Default, Debug)]
struct VoteTemplateContext<'a> {
    items: Vec<(&'a str, &'a str)>,
    // This key tells handlebars which template is the parent.
    parent: &'a str,
}


#[get("/")]
fn index() -> Template {
    Template::render("index", &VoteTemplateContext {
        items: PLACES.clone().into_iter().collect_vec(),
        parent: "layout",
    })

}

#[get("/results")]
fn results(db_conn: State<DbConn>) -> Template {
    let votes: Vec<Vote> = db_conn.lock()
        .expect("db connection lock")
        .prepare("SELECT username, place FROM vote_results where date = $1")
        .unwrap()
        .query_map(&[&Local::today().naive_local() as &ToSql], |row| {
            let username: String = row.get(0);
            let place: String = row.get(1);

            let vote = Vote {
                username,
                place
            };

            vote
        }).unwrap()
        .map(|tv| tv.unwrap()).collect_vec();

    let mut frequency: HashMap<_, _> = HashMap::new();
    for word in &votes {
        *frequency.entry(word.place.as_str()).or_insert(0) += 1;
    }
    //sorting
    let sorted_frequency = frequency.into_iter().sorted_by_key(|x| -x.1);

    Template::render("results", &TemplateContext {
        frequency: format!("{:#?}", sorted_frequency).as_str(),
        votes: format!("{:#?}", votes).as_str(),
        parent: "layout",
    })
}

#[derive(Debug, FromForm)]
struct FormInput<'r> {
    username: &'r RawStr,
    markthalle: bool,
    burgerlich: bool,
    andronaco: bool,
    hans_in_glueck: bool,
    thai_food: bool,
    wildes_fraeulein: bool,
    sala_thai: bool,
    galette_de_bretagne: bool,
    mozzers: bool,
    chie_tu_huang: bool,
    kartoffelkeller: bool,
    o_ren_ishii: bool,
    dos_amigos: bool,
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

            if form.markthalle {places.push(PLACES.get("markthalle").unwrap())};
            if form.burgerlich {places.push(PLACES.get("burgerlich").unwrap())};
            if form.andronaco {places.push(PLACES.get("andronaco").unwrap())};
            if form.hans_in_glueck {places.push(PLACES.get("hans_in_glueck").unwrap())};
            if form.thai_food {places.push(PLACES.get("thai_food").unwrap())};
            if form.wildes_fraeulein {places.push(PLACES.get("wildes_fraeulein").unwrap())};
            if form.sala_thai {places.push(PLACES.get("sala_thai").unwrap())};
            if form.galette_de_bretagne {places.push(PLACES.get("galette_de_bretagne").unwrap())};
            if form.mozzers {places.push(PLACES.get("mozzers").unwrap())};
            if form.chie_tu_huang {places.push(PLACES.get("chie_tu_huang").unwrap())};
            if form.kartoffelkeller {places.push(PLACES.get("kartoffelkeller").unwrap())};
            if form.o_ren_ishii {places.push(PLACES.get("o_ren_ishii").unwrap())};
            if form.dos_amigos {places.push(PLACES.get("dos_amigos").unwrap())};

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