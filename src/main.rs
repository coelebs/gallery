#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate image;
extern crate libraw_sys as libraw;
extern crate base64;
extern crate quick_xml;
extern crate rusqlite;
extern crate rocket;
extern crate time;

pub mod model;

use rusqlite::Connection;

use std::fs;
use std::path;
use std::env;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/gallery/<rating>")]
fn gallery(rating: u8) -> String {
    let mut result = String::from("<html><head/><body>");

    let conn = Connection::open("db.sqlite").ok().unwrap();
    let mut qry = conn.prepare("SELECT id, rating, path FROM model::Image WHERE rating = ?1").unwrap();

    let image_iter = qry.query_map(&[&rating], |row| model::Image::from_row(row)).unwrap();
    
    let mut subj_qry = conn.prepare("SELECT s.id, s.family, person FROM model::Subject s
                            LEFT JOIN model::Image_model::Subjects ims ON ims.subject_id = s.id
                            WHERE ims.image_id = ?1").unwrap();
    for i in image_iter {
        let image = i.unwrap();
        result.push_str(&format!("{:?}\n", image.rating));

        let subject_iter = subj_qry.query_map(&[&image.id], |row| {
            model::Subject {
                id: row.get(0),
                family: row.get(1),
                person: row.get(2),
            }
        }).unwrap();

        for s in subject_iter {
            let subject = s.unwrap();
            result.push_str(&format!("\t\t{:?}|{:?}\n", subject.family, subject.person));
        }

        result.push('\n');
    }
     
    result.push_str("</body></html>");

    result
}

fn read_dir(entry_point: &path::Path, conn: &rusqlite::Connection) {
    for entry in fs::read_dir(entry_point).unwrap() {
        let path = entry.ok().unwrap().path();
        if path.is_file() && path.extension().unwrap().to_str() == Some("CR2") {
            println!("Scanning {:?}", path);
            model::Image::parse(&path, &conn).insert(&conn);
        } else if path.is_dir() {
            read_dir(&path, &conn);
        }
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let entry_point = path::Path::new("/mnt/freenas/pictures/2018/");

    if args.len() > 1 {
        let conn = Connection::open("db.sqlite").ok().unwrap();
        model::Image::initialize_db(&conn);
        model::Subject::initialize_db(&conn);
        
        read_dir(&entry_point, &conn);
    }

    rocket::ignite().mount("/", routes![index, gallery]).launch();
}
