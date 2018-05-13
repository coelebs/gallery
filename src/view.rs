use model;

use rocket::response::NamedFile;
use rocket_contrib::Template;

use rusqlite::Connection;

use std::path;

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
