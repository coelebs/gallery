use image;
use libraw;
use std;
use chrono;
use schema;
use diesel;

use uuid::Uuid;

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

use super::schema::images;

#[derive(Insertable)]
#[table_name="images"]
pub struct NewImage<'a> {
    pub path: &'a str,
    pub rating: i32,
    pub last_modified: chrono::NaiveDateTime,
    pub thumb_path: &'a str, 
}

#[derive(Debug,Serialize,Queryable,Clone)]
pub struct Image {
    pub id: i32,
    pub path: String,
    pub rating: i32,
    //pub subjects: Vec<Subject>,
    pub last_modified: chrono::NaiveDateTime,
    pub thumb_path: String,
}

#[derive(Debug,Serialize,Queryable)]
pub struct Subject {
    pub id: i32,
    pub family: String,
    pub person: String,
}

#[derive(Debug,Serialize,Queryable)]
pub struct ImageSubjects {
  pub id: i64,        
  pub image_id: i64,  
  pub subject_id: i64,
}

impl Image {
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
        let mut subject = false;
        let mut subjects = Vec::new();
        loop {
            match reader.read_event(&mut buf) {
                Ok(Event::Start(ref e)) => 
                    match e.name() {
                        b"rdf:Description" => rating = Image::parse_rating(
                                                            e.attributes()
                                                            .map(|a| a.unwrap()).collect::<Vec<_>>(), &reader
                                                            ),
                        b"lr:hierarchicalSubject" => subject = true,
                        _ => (),
                    },
                Ok(Event::End(ref e)) => 
                    match e.name() {
                        b"lr:hierarchicalSubject" => subject = false,
                        _ => (),
                    },
                Ok(Event::Eof) => break,
                Ok(Event::Text(ref e)) => if subject {subjects.push(e.unescape_and_decode(&reader).ok().unwrap())},
                _ => (),
            }

            buf.clear();
        }

        subjects.retain(|x| x.trim().len() > 0);

        let thumb_path = Image::develop_thumb(img_path, thumb_dir);
        let system_time = img_path.metadata().unwrap()
                                  .modified().unwrap()
                                  .duration_since(std::time::UNIX_EPOCH).unwrap();

        let new_image = NewImage {
            path: img_path.to_str().unwrap(),
            rating: rating.unwrap(), 
            last_modified: chrono::NaiveDateTime::from_timestamp_opt(system_time.as_secs() as i64,
                                                                     system_time.subsec_millis()).unwrap(),
            thumb_path: thumb_path.to_str().unwrap()
        };

        diesel::insert_into(schema::images::table)
            .values(&new_image)
            .get_result(conn)
            .expect("Error saving new post")
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

impl Subject {
    fn parse_subjects(input: &Vec<String>) -> Vec<Subject> {
        let mut result = Vec::new();
        for x in input {
            let mut iter = x.rsplit("|");
            let person = iter.next().unwrap();
            let family = iter.next().unwrap();
            result.push(Subject{id:-1, family: String::from(family), person: String::from(person)});
        }

        result
    }
}

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

