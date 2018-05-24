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
    rating: i32,
    from: String,
    to: String,
    tags: String,
    page: i64,
}

#[derive(Debug, FromForm, Clone, Serialize)]
struct Input {
    rating: Option<i32>,
    from: Option<String>,
    to: Option<String>,
    tags: Option<String>,
}

impl GalleryTemplate {
    fn new(input: Option<Input>, page: i64) -> GalleryTemplate {
        if input.is_none() {
            GalleryTemplate {
                title: String::from("rawgallery"), 
                rating: 1,
                to: String::new(),
                from: String::new(),
                tags: String::new(),
                images: Vec::new(),
                page: page
            }
        } else {
            let unwrapped = input.unwrap();
            GalleryTemplate {
                title: String::from("rawgallery"), 
                rating: unwrapped.rating.unwrap_or(1),
                to: unwrapped.to.unwrap_or(String::new()),
                from: unwrapped.from.unwrap_or(String::new()),
                tags: unwrapped.tags.unwrap_or(String::new()),
                images: Vec::new(),
                page: page
            }
        }
    }
}

impl Input {
    fn parsed_from(&self) -> chrono::NaiveDateTime {
        if self.from.is_some() && self.from.clone().unwrap().len() > 0 {
            chrono::NaiveDate::parse_from_str(&self.from.clone().unwrap(), "%Y-%m-%d")
                .unwrap_or(chrono::NaiveDate::from_yo(1970, 1))
                .and_hms(0,0,0)
        }  else {
            chrono::NaiveDateTime::from_timestamp(0, 0)
        }
    }

    fn parsed_to(&self) -> chrono::NaiveDateTime {
        if self.to.is_some() && self.to.clone().unwrap().len() > 0 {
            chrono::NaiveDate::parse_from_str(&self.to.clone().unwrap(), "%Y-%m-%d")
                .unwrap_or(chrono::NaiveDate::from_yo(2170, 1))
                .and_hms(23, 59, 59)
        }  else {
            chrono::Utc::now().naive_local()
        }
    }
}

#[get("/")]
fn index() -> &'static str {
    "index"
}

#[get("/gallery?<filter>")]
fn gallery(filter: Option<Input>) -> Template {
    filter_gallery(0, filter)
}

#[get("/gallery/<page>?<filter>")]
fn filter_gallery(page: i64, filter: Option<Input>) -> Template {
    use schema::images::dsl::*;
    use schema::image_tags::dsl::*;
    use schema::tags::dsl::*;

    let connection = model::establish_connection();

    let mut context = GalleryTemplate::new(filter.clone(), page);

    let imgs;
    let mut tags_input = None;
    if let Some(raw_filter) = filter {
        tags_input = raw_filter.clone().tags;

        imgs = images
            .order(datetime.asc())
            .filter(rating.ge(raw_filter.rating.unwrap_or(0)))
            .filter(datetime.gt(raw_filter.clone().parsed_from()))
            .filter(datetime.lt(raw_filter.clone().parsed_to()))
            .offset(50 * page)
            .limit(50)
            .load::<model::Image>(&connection)
            .expect("Error filtering and loading images");
    }  else {
        imgs = images
            .order(datetime.asc())
            .offset(50 * page)
            .limit(50)
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

    println!("Len: {:?}", context.images.len());
     
    Template::render("index", &context)
}

#[get("/static/<file..>")]
fn files(file: path::PathBuf) -> Option<NamedFile> {
    NamedFile::open(path::Path::new("static/").join(file)).ok()
}
