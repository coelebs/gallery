use model;
use schema;
use chrono;

use rocket::response::NamedFile;
use rocket_contrib::Template;

use std::path;

use diesel::prelude::*;

#[derive(Serialize)]
struct GalleryTemplate {
    title: String,
    images: Vec<(model::Image, Vec<model::Tag>)>,
}

#[derive(Debug, FromForm)]
struct Filter {
    rating: Option<i32>,
    from: Option<String>,
    to: Option<String>,
    tags: Option<String>,
}


#[get("/")]
fn index() -> &'static str {
    "index"
}

#[get("/gallery")]
fn gallery() -> Template {
    filter_gallery(None)
}

#[get("/gallery?<filter>")]
fn filter_gallery(filter: Option<Filter>) -> Template {
    use schema::images::dsl::*;
    use schema::image_tags::dsl::*;
    use schema::tags::dsl::*;

    let connection = model::establish_connection();
    let mut context = GalleryTemplate { title: String::from("rawgallery"), images: Vec::new() };

    let imgs;
    let mut tags_input = None;
    if let Some(raw_filter) = filter {
        tags_input = raw_filter.tags; 


        let from_filter;
        if raw_filter.from.is_some() && raw_filter.from.clone().unwrap().len() > 0 {
            from_filter = datetime.gt(
                chrono::NaiveDate::parse_from_str(&raw_filter.from.unwrap(), "%Y-%m-%d").unwrap().and_hms(0,0,0));
        }  else {
            from_filter = datetime.gt(
                chrono::NaiveDateTime::from_timestamp(0, 0));
        }

        let to_filter;
        if raw_filter.to.is_some() && raw_filter.to.clone().unwrap().len() > 0 {
            to_filter = datetime.lt(
                chrono::NaiveDate::parse_from_str(&raw_filter.to.unwrap(), "%Y-%m-%d").unwrap().and_hms(23, 59, 59));
        }  else {
            to_filter = datetime.lt(
                chrono::NaiveDateTime::from_timestamp(chrono::Utc::now().timestamp(), 0));
        }

        imgs = images
            .filter(rating.ge(raw_filter.rating.unwrap_or(0)))
            .filter(to_filter)
            .filter(from_filter)
            .load::<model::Image>(&connection)
            .expect("Error filtering and loading images");
    }  else {
        imgs = images
            .load::<model::Image>(&connection)
            .expect("Error loading imaages");
    }



    for image in imgs.clone() {
        let tag_query = image_tags
            .inner_join(tags)
            .filter(image_id.eq(image.id));

        let tag_list;
        if tags_input.is_some() && tags_input.clone().unwrap().len() > 0 {
           let local_tags = tags_input.clone().unwrap(); 
           let filter_tags: Vec<&str> = local_tags.split(';').collect();

           tag_list = tag_query
               .filter(content.overlaps_with(filter_tags))
               .select((schema::tags::id, schema::tags::content))
               .load::<model::Tag>(&connection)
               .expect("Error loading tags");
        } else {
            tag_list = tag_query
                .select((schema::tags::id, schema::tags::content))
                .load::<model::Tag>(&connection)
                .expect("Error loading tags");
        }

        if tag_list.len() > 0 {
            context.images.push((image, tag_list));
        }
    }
     
    Template::render("index", &context)
}

#[get("/static/<file..>")]
fn files(file: path::PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new("static/").join(file)).ok()
}
