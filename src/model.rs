use image;
use libraw;
use std;
use chrono;
use schema;
use diesel; 
use rexiv2;

use quick_xml::reader::Reader;
use quick_xml::events::Event;
use quick_xml::events::attributes::Attribute;

use std::io::BufReader;
use std::fs;
use std::path;
use std::ffi::CString;
use std::process::Command;

use diesel::prelude::*;
use diesel::Connection;

use dotenv::dotenv;

use uuid::Uuid;

use super::schema::images;
use super::schema::image_tags;
use super::schema::tags;

#[derive(Insertable)]
#[table_name="images"]
pub struct NewImage<'a> {
    pub path: &'a str,
    pub rating: i32,
    pub last_modified: chrono::NaiveDateTime,
    pub thumb_path: &'a str, 
    pub datetime: chrono::NaiveDateTime,
}

#[derive(Insertable)]
#[table_name="image_tags"]
pub struct NewImageTag {
    pub image_id: i32,
    pub tag_id: i32,
}

#[derive(Insertable)]
#[table_name="tags"]
pub struct NewTag<'a> {
    pub content: Vec<&'a str>,
}

#[derive(Identifiable,Debug,Serialize,Queryable,Clone,Associations)]
#[table_name="images"]
pub struct Image {
    pub id: i32,
    pub path: String,
    pub rating: i32,
    pub last_modified: chrono::NaiveDateTime,
    pub thumb_path: String,
    pub datetime: chrono::NaiveDateTime,
}

#[derive(Identifiable,Debug,Serialize,Queryable,Clone,Associations)]
#[table_name="tags"]
pub struct Tag {
    pub id: i32, 
    pub content: Vec<String>,
}

#[derive(Identifiable,Debug,Serialize,Queryable,Associations)]
#[belongs_to(Image)]
#[belongs_to(Tag)]
#[table_name="image_tags"]
pub struct ImageTag {
  pub id: i32,        
  pub image_id: i32,  
  pub tag_id: i32,
}

impl Image {
    fn new(path: &path::Path, 
           rating: i32, 
           thumb_dir: &path::Path, 
           mut tags: Vec<String>,
           conn: &PgConnection) -> Image {
        let exiv = rexiv2::Metadata::new_from_path(&path).unwrap();
        let image_date = chrono::NaiveDateTime::parse_from_str(
            &exiv.get_tag_string("Exif.Image.DateTime").unwrap(),
            "%Y:%m:%d %H:%M:%S").unwrap();

        let thumb_path = Image::develop_thumb(path, thumb_dir);
        let system_time = path.metadata().unwrap()
                                  .modified().unwrap()
                                  .duration_since(std::time::UNIX_EPOCH).unwrap();


        let new_image = NewImage {
            path: path.to_str().unwrap(),
            rating: rating, 
            last_modified: chrono::NaiveDateTime::from_timestamp_opt(system_time.as_secs() as i64,
                                                                     system_time.subsec_millis()).unwrap(),
            thumb_path: thumb_path.to_str().unwrap(),
            datetime: image_date
        };

        let image = diesel::insert_into(schema::images::table)
            .values(&new_image)
            .get_result(conn)
            .expect("Error saving new post");

        tags.retain(|x| x.trim().len() > 0);

        Tag::parse(&mut tags, conn)
            .into_iter()
            .for_each(|x| x.attach_to_image(&image, conn));

        image
    }

    fn parse_rating(input: Vec<Attribute>, reader: &Reader<BufReader<fs::File>>) -> Option<i32> {
        input.into_iter().filter(|x| x.key == b"xmp:Rating")
              .map(|x| x.unescape_and_decode_value(reader).ok().unwrap().parse().unwrap())
              .last()
    }

    fn parse_xmp(img_path: &path::Path, thumb_dir: &path::Path, conn: &PgConnection) -> Image {
        let xmp = img_path.with_extension(format!("{}.xmp", img_path.extension().unwrap()
                                                            .to_str().unwrap()));

        let mut reader = 
            Reader::from_file(xmp).ok().unwrap();

        let mut rating = None;
        let mut buf = Vec::new();
        let mut tag = false;
        let mut raw_tags = Vec::new();
        loop {
            match reader.read_event(&mut buf) {
                Ok(Event::Start(ref e)) => 
                    match e.name() {
                        b"rdf:Description" => rating = Image::parse_rating(
                                                            e.attributes()
                                                            .map(|a| a.unwrap()).collect::<Vec<_>>(), &reader
                                                            ),
                        b"lr:hierarchicalSubject" => tag = true,
                        _ => (),
                    },
                Ok(Event::End(ref e)) => 
                    match e.name() {
                        b"lr:hierarchicalSubject" => tag = false,
                        _ => (),
                    },
                Ok(Event::Eof) => break,
                Ok(Event::Text(ref e)) => if tag {raw_tags.push(e.unescape_and_decode(&reader).ok().unwrap())},
                _ => (),
            }

            buf.clear();
        }

        Image::new(img_path, rating.unwrap(), thumb_dir, raw_tags, conn)
    }

    pub fn parse(path: &path::Path, thumb_dir: &path::Path, conn: &PgConnection) -> Image {
        info!("Parsing {:?}", path);

        let result;

        let images  = images::table.filter(images::path.eq(path.to_str().unwrap()))
                        .load::<Image>(conn)
                        .unwrap();
        
        if images.len() == 0 {
            result = Image::parse_xmp(path, thumb_dir, conn);
        } else {
            let image = &images[0];

            if (image.last_modified.timestamp() as u64)
                  < path.metadata().unwrap().modified().unwrap()
                        .duration_since(std::time::UNIX_EPOCH).unwrap() 
                        .as_secs() {
                result = Image::parse_xmp(path, thumb_dir, conn); 
            } else {
                result = image.clone();
            }
        }

        result
    }

    fn extract_thumb(raw_path: &path::Path, thumb_path: &path::Path) -> path::PathBuf {
        let thumb_data;
        unsafe {
            let libraw_data = libraw::libraw_init(libraw::LIBRAW_OPTIONS_NONE);
            
            if libraw::libraw_open_file(libraw_data, 
                                        CString::new(raw_path.to_str().unwrap()).unwrap().as_ptr()) != 0 {
                panic!("Libraw open file failed");
            }

            if libraw::libraw_unpack_thumb(libraw_data) != 0 {
                panic!("Libraw unpack thumb failed");
            }

            let mut result = 0;
            let libraw_thumb = libraw::libraw_dcraw_make_mem_thumb(libraw_data, &mut result);
            if result != 0 {
                panic!("Libraw make mem thumb failed");
            }

            thumb_data = std::slice::from_raw_parts((*libraw_thumb).data.as_ptr(), 
                                                    (*libraw_thumb).data_size as usize);
        }

        let mut img = image::load_from_memory(thumb_data).ok().unwrap();
        img = img.thumbnail(1000, 1000);

        let thumb_file = thumb_path.to_path_buf().join(format!("{}.jpg", 
                                                       Uuid::new_v4().hyphenated()));

        img.save(thumb_file.clone()).unwrap();

        thumb_file
    }

    fn develop_thumb(raw_path: &path::Path, thumb_path: &path::Path) -> path::PathBuf {
        let xmp = raw_path.with_extension(format!("{}.xmp", raw_path.extension().unwrap()
                                                            .to_str().unwrap()));
        
        let thumb_file = thumb_path.to_path_buf().join(format!("{}.jpg", 
                                                       Uuid::new_v4().hyphenated()));

        let output = Command::new("darktable-cli")
                             .arg(raw_path.to_str().unwrap())
                             .arg(xmp.to_str().unwrap())
                             .arg(thumb_file.to_str().unwrap()) 
                             .args(&["--width", "640"])
                             .args(&["--height", "640"])
                             .output()
                             .expect("Failed to develop image");

        info!("Darktable stdout: {}", String::from_utf8_lossy(&output.stdout));
        info!("Darktable stderr: {}", String::from_utf8_lossy(&output.stderr));

        thumb_file
    }
}

impl Tag {
    fn parse(input: &mut Vec<String>, conn: &PgConnection) -> Vec<Tag> {
        let mut result = Vec::new();
        
        for x in input {
            let content: Vec<&str> = x.split("|").collect();

            let tags = tags::table
                .filter(tags::content.eq(content.clone()))
                .load::<Tag>(conn)
                .expect("Error loading Tag");

            if tags.len() == 0 {
                let new_tag = NewTag { 
                    content,
                };

                result.push(diesel::insert_into(schema::tags::table)
                    .values(&new_tag)
                    .get_result(conn)
                    .expect("Error saving Tag"));
            } else {
                result.push(tags[0].clone());
            }
        }

        result
    }

    fn attach_to_image(&self, image: &Image, conn: &PgConnection) {
        let new_image_tag = NewImageTag {
            image_id: image.id,
            tag_id: self.id,
        };

        diesel::insert_into(schema::image_tags::table)
            .values(&new_image_tag)
            .execute(conn)
            .expect("Error associating image and tag");
    }
}

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

