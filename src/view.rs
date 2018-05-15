use model;
use schema;

use rocket::response::NamedFile;
use rocket_contrib::Template;

use std::path;

use diesel::prelude::*;

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
fn gallery(rating: i32) -> Template {

    let connection = model::establish_connection();

    let images = schema::images::table
                         .filter(schema::images::rating.eq(rating))
                         .load::<model::Image>(&connection)
                         .expect("Error loading imaages");


    let context = GalleryTemplate { title: String::from("rawgallery"), images: images };
     
    Template::render("index", &context)
}

#[get("/static/<file..>")]
fn files(file: path::PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new("static/").join(file)).ok()
}
