use model;
use schema;

use rocket::response::NamedFile;
use rocket_contrib::Template;

use std::path;

use diesel::prelude::*;

#[derive(Serialize)]
struct GalleryTemplate {
    title: String,
    images: Vec<(model::Image, Vec<model::Tag>)>,
}

#[get("/")]
fn index() -> &'static str {
    "index"
}

#[get("/gallery/<input>")]
fn gallery(input: i32) -> Template {
    use schema::images::dsl::*;
    use schema::image_tags::dsl::*;
    use schema::tags::dsl::*;

    let connection = model::establish_connection();
    let mut context = GalleryTemplate { title: String::from("rawgallery"), images: Vec::new() };

    let imgs = images.filter(rating.eq(input))
                       .order_by(datetime.asc())
                       .load::<model::Image>(&connection)
                       .expect("Error loading imaages");

    for image in imgs.clone() {
        let tag_list = image_tags
            .inner_join(tags)
            .filter(image_id.eq(image.id))
            .select((schema::tags::id, schema::tags::content))
            .load::<model::Tag>(&connection)
            .expect("Error loading tags");

        context.images.push((image, tag_list));
    }


     
    Template::render("index", &context)
}

#[get("/static/<file..>")]
fn files(file: path::PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new("static/").join(file)).ok()
}
