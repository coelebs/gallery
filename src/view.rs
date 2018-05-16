use model;
use schema;

use rocket::response::NamedFile;
use rocket_contrib::Template;

use std::path;

use diesel::prelude::*;

#[derive(Serialize)]
struct GalleryTemplate {
    title: String,
    images: Vec<(model::Image, Vec<model::Subject>)>,
}

#[get("/")]
fn index() -> &'static str {
    "index"
}

#[get("/gallery/<input>")]
fn gallery(input: i32) -> Template {
    use schema::images::dsl::*;
    use schema::subjects::dsl::*;
    use schema::image_subjects::dsl::*;

    let connection = model::establish_connection();
    let mut context = GalleryTemplate { title: String::from("rawgallery"), images: Vec::new() };

    let imgs = images.filter(rating.eq(input))
                       .load::<model::Image>(&connection)
                       .expect("Error loading imaages");

    for image in imgs.clone() {
        let tags = image_subjects
            .inner_join(subjects)
            .filter(image_id.eq(image.id))
            .select((schema::subjects::id, family, person))
            .load::<model::Subject>(&connection)
            .expect("Error loading subjects");

        context.images.push((image, tags));
    }


     
    Template::render("index", &context)
}

#[get("/static/<file..>")]
fn files(file: path::PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new("static/").join(file)).ok()
}
