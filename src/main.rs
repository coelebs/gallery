#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate image;
extern crate libraw_sys as libraw;
extern crate quick_xml;
extern crate rusqlite;
extern crate rocket;
extern crate rocket_contrib;
extern crate time;
extern crate uuid;
extern crate env_logger;
extern crate serde;

#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;

pub mod model;

use rusqlite::Connection;

use std::fs;
use std::path;
use std::env;

use rocket::response::NamedFile;
use rocket_contrib::Template;

#[derive(Serialize)]
struct GalleryTemplate {
    title: String,
    images: Vec<model::Image>,
}

#[get("/")]
fn index() -> &'static str {
    "index"
}

#[get("/gallery/<rating>")]
fn gallery(rating: u8) -> Template {
    let mut context = GalleryTemplate { title: String::from("rawgallery"), images: Vec::new()};
    let conn = Connection::open("db.sqlite").ok().unwrap();
    let mut qry = conn.prepare("SELECT * FROM Image WHERE rating = ?1").unwrap();

    let image_iter = qry.query_map(&[&rating], |row| model::Image::from_row(row)).unwrap();
    
    let mut subj_qry = conn.prepare("SELECT s.id, s.family, person FROM Subject s
                            LEFT JOIN Image_Subjects ims ON ims.subject_id = s.id
                            WHERE ims.image_id = ?1").unwrap();
    for i in image_iter {
        let mut image = i.unwrap();

        subj_qry.query_map(&[&image.id], |row| {
            model::Subject {
                id: row.get(0),
                family: row.get(1),
                person: row.get(2),
            }
        }).unwrap()
        .for_each(|x| image.subjects.push(x.unwrap()));

        context.images.push(image); 
    }
     
    Template::render("index", &context)
}

#[get("/static/<file..>")]
fn files(file: path::PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new("static/").join(file)).ok()
}

fn read_dir(entry_point: &path::Path, thumb_dir: &path::Path, conn: &rusqlite::Connection) {
    info!("Scanning folder: {:?}", entry_point);
    for entry in fs::read_dir(entry_point).unwrap() {
        let path = entry.ok().unwrap().path();
        if path.is_file() && path.extension().unwrap()
                                 .to_str().unwrap().to_uppercase() == "CR2" {
            model::Image::parse(&path, &thumb_dir, &conn).insert(&conn);
        } else if path.is_dir() {
            read_dir(&path, &thumb_dir, &conn);
        }
    }
}

fn main() {
    env_logger::init();

    let args: Vec<_> = env::args().collect();
    let entry_point = path::Path::new("/mnt/freenas/pictures/2018/2018-05/2018-05-11/");
    let thumb_dir   = path::Path::new("static/thumb/");

    if args.len() > 1 {
        info!("Starting scan over {:?}", entry_point);
        info!("Saving thumbnails in {:?}", thumb_dir);

        let conn = Connection::open("db.sqlite").ok().unwrap();
        model::Image::initialize_db(&conn);
        model::Subject::initialize_db(&conn);
        
        read_dir(&entry_point, &thumb_dir, &conn);
    }

    rocket::ignite()
        .mount("/", routes![index, gallery, files])
        .attach(Template::fairing())
        .launch();
}
