#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate image;
extern crate libraw_sys as libraw;
extern crate quick_xml;
extern crate rocket;
extern crate rocket_contrib;
extern crate chrono;
extern crate uuid;
extern crate env_logger;
extern crate serde;
extern crate clap;
extern crate dotenv;
extern crate rexiv2;

#[macro_use] extern crate diesel;
#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;

use std::fs;
use std::path;

pub mod model;
pub mod schema;
pub mod view;

fn read_dir(entry_point: &path::Path, thumb_dir: &path::Path, conn: &diesel::PgConnection) {
    info!("Scanning folder: {:?}", entry_point);
    for entry in fs::read_dir(entry_point).unwrap() {
        let path = entry.ok().unwrap().path();
        if path.is_file() && path.extension().unwrap()
                                 .to_str().unwrap().to_uppercase() == "CR2" {
            model::Image::parse(&path, &thumb_dir, &conn);
        } else if path.is_dir() {
            read_dir(&path, &thumb_dir, &conn);
        }
    }
}

fn main() {
    env_logger::init();

    let matches = clap::App::new("RawGallery")
                        .subcommand(clap::SubCommand::with_name("scan")
                                          .about("Scan directory and add to gallery")
                                          .version("0.1")
                                          .arg(clap::Arg::with_name("FOLDER")
                                               .help("Start recursively scanning from this folder")
                                               .required(true)
                                               .index(1)))
                        .get_matches();

    let thumb_dir   = path::Path::new("static/thumb/");

    if let Some(matches) = matches.subcommand_matches("scan") {
        let entry_point = path::Path::new(matches.value_of("FOLDER").unwrap());

        info!("Starting scan over {:?}", entry_point);
        info!("Saving thumbnails in {:?}", thumb_dir);

        let conn = model::establish_connection();
        
        read_dir(&entry_point, &thumb_dir, &conn);
    }

    rocket::ignite()
        .mount("/", routes![view::index, view::gallery, view::files])
        .attach(rocket_contrib::Template::fairing())
        .launch();
}
