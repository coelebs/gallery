use image;
use libraw;
use rusqlite;
use std;
use time;

use uuid::Uuid;

use quick_xml::reader::Reader;
use quick_xml::events::Event;
use quick_xml::events::attributes::Attribute;

use std::io::BufReader;
use std::fs;
use std::path;
use std::ffi::CString;
use std::process::Command;

#[derive(Serialize)]
#[serde(remote = "time::Timespec")]
struct TimespecDef {
    sec: i64,
    nsec: i32,
}

#[derive(Debug,Serialize)]
pub struct Image {
    pub id: i64,
    pub path: path::PathBuf,
    pub rating: u8,
    pub subjects: Vec<Subject>,
    #[serde(with = "TimespecDef")]
    pub last_modified: time::Timespec,
    pub thumb_path: path::PathBuf,
}

#[derive(Debug,Serialize)]
pub struct Subject {
    pub id: i64,
    pub family: String,
    pub person: String,
}

impl Image {
    pub fn initialize_db(conn: &rusqlite::Connection) {

        conn.execute("CREATE TABLE IF NOT EXISTS Image (
                        id              INTEGER PRIMARY KEY,
                        path            TEXT,
                        rating          INTEGER,
                        last_modified   TEXT,
                        thumb_path      TEXT
                      );", &[]).unwrap();

        conn.execute("CREATE TABLE IF NOT EXISTS Image_Subjects (
                        img_subj_id INTEGER PRIMARY KEY, 
                        image_id    INTEGER,
                        subject_id  INGEGER
                      );", &[]).unwrap();
    }

    pub fn insert(self, conn: &rusqlite::Connection) -> i64 {
        conn.execute("INSERT INTO Image (path, rating, last_modified, thumb_path)
                      VALUES (?1, ?2, ?3, ?4)", 
                      &[&self.path.to_str(), &self.rating, &self.last_modified, &self.thumb_path.to_str()])
                    .unwrap();

        let image_id = conn.last_insert_rowid();

        for mut subject in self.subjects {
            subject.insert(conn);

            conn.execute("INSERT INTO Image_Subjects (image_id, subject_id)
                          VALUES (?1, ?2)", &[&image_id, &subject.id]).unwrap();
        }

        image_id
    }

    pub fn from_row(row: &rusqlite::Row) -> Image {
         Image {
            id: row.get(0),
            path: path::PathBuf::from(row.get::<i32, String>(1)),
            rating: row.get(2),
            subjects: Vec::new(),
            last_modified: row.get(3),
            thumb_path: path::PathBuf::from(row.get::<i32, String>(4))
        }
    }

    fn parse_rating(input: Vec<Attribute>, reader: &Reader<BufReader<fs::File>>) -> Option<u8> {
        input.into_iter().filter(|x| x.key == b"xmp:Rating")
              .map(|x| x.unescape_and_decode_value(reader).ok().unwrap().parse::<u8>().unwrap())
              .last()
    }

    fn parse_xmp(img_path: &path::Path, thumb_dir: &path::Path) -> Image {
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

        let thumb_path =  Image::develop_thumb(img_path, thumb_dir);

        Image {
            id: -1, 
            path: img_path.to_path_buf(), 
            rating: rating.unwrap(), 
            subjects: Subject::parse_subjects(&subjects),
            last_modified: time::now().to_timespec(),
            thumb_path: thumb_path
        }
    }

    pub fn parse(path: &path::Path, thumb_dir: &path::Path,  conn: &rusqlite::Connection) -> Image {
        info!("Parsing {:?}", path);

        let result;

        let mut query = conn.prepare("SELECT * FROM Image
                                      WHERE path = ?1").unwrap();
        let mut image_iter = query.query_map(&[&path.to_path_buf().to_str()], |row| Image::from_row(row))
                                  .unwrap();

        let nxt = image_iter.next();
        
        if nxt.is_none() {
            result = Image::parse_xmp(path, thumb_dir);
        } else {
            let image = nxt.unwrap().unwrap();

            if (image.last_modified.sec as u64) 
                  < path.metadata().unwrap().modified().unwrap()
                        .duration_since(std::time::UNIX_EPOCH).unwrap() 
                        .as_secs() {
                result = Image::parse_xmp(path, thumb_dir); 
            } else {
                result = image;
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
                             .arg(raw_path.as_os_str())
                             .arg(xmp.as_os_str())
                             .arg(thumb_file.as_os_str())
                             .arg("--width 1000")
                             .arg("--height 1000")
                             .output()
                             .expect("Failed to develop image");

        info!("Darktable stdout: {}", String::from_utf8_lossy(&output.stdout));
        info!("Darktable stderr: {}", String::from_utf8_lossy(&output.stderr));

        thumb_file
    }
}

impl Subject {
    pub fn initialize_db(conn: &rusqlite::Connection) {
        conn.execute("CREATE TABLE IF NOT EXISTS Subject (
                        id          INTEGER PRIMARY KEY,
                        family      TEXT,
                        person      TEXT
                    );", &[]).unwrap();
    }

    pub fn insert(&mut self, conn: &rusqlite::Connection) {
        let id = conn.query_row("SELECT id, family, person FROM Subject
                                WHERE family = ?1 and person = ?2", 
                                &[&self.family, &self.person], |x| { x.get(0)});

        self.id = if id.is_ok() {
            id.unwrap()
        } else {
            conn.execute("INSERT INTO Subject (family, person) 
                          VALUES (?1, ?2)", &[&self.family, &self.person]).unwrap();

            conn.last_insert_rowid()
        };
    }
    
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
