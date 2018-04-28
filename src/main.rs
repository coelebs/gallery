extern crate quick_xml;
extern crate rusqlite;

use quick_xml::reader::Reader;
use quick_xml::events::Event;
use quick_xml::events::attributes::Attribute;

use rusqlite::Connection;

use std::io::BufReader;
use std::fs;
use std::path;

#[derive(Debug)]
struct Image {
    path: path::PathBuf,
    rating: u8,
    subjects: Vec<Subject>,
}

#[derive(Debug)]
struct Subject {
    id: i64,
    family: String,
    person: String,
}

impl Image {
    fn initialize_db(conn: &Connection) {
        conn.execute("CREATE TABLE Image (
                        id          INTEGER PRIMARY KEY,
                        path        TEXT,
                        rating      INTEGER
                      );", &[]).unwrap();

        conn.execute("CREATE TABLE Image_Subjects (
                        img_subj_id INTEGER PRIMARY KEY, 
                        image_id    INTEGER,
                        subject_id  INGEGER
                      );", &[]).unwrap();
    }

    fn insert(self, conn: &Connection) -> i64 {
        conn.execute("INSERT INTO Image (path, rating)
                      VALUES (?1, ?2)", &[&self.path.to_str(), &self.rating]).unwrap();

        let image_id = conn.last_insert_rowid();

        for mut subject in self.subjects {
            subject.insert(conn);

            conn.execute("INSERT INTO Image_Subjects (image_id, subject_id)
                          VALUES (?1, ?2)", &[&image_id, &subject.id]).unwrap();
        }

        image_id
    }

    fn parse_rating(input: Vec<Attribute>, reader: &Reader<BufReader<fs::File>>) -> Option<u8> {
        input.into_iter().filter(|x| x.key == b"xmp:Rating")
              .map(|x| x.unescape_and_decode_value(reader).ok().unwrap().parse::<u8>().unwrap())
              .last()
    }

    fn parse_xmp(path: &path::Path) -> Image {
        let mut reader = 
            Reader::from_file(path).ok().unwrap();

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

        Image {path: path.to_path_buf(), rating: rating.unwrap(), subjects: Subject::parse_subjects(&subjects)}
    }
}

impl Subject {
    fn initialize_db(conn: &Connection) {
        conn.execute("CREATE TABLE Subject (
                        id          INTEGER PRIMARY KEY,
                        family      TEXT,
                        person      TEXT
                    );", &[]).unwrap();
    }

    fn insert(&mut self, conn: &Connection) {
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

fn main() {
    let entry_point = path::Path::new("/mnt/freenas/pictures/organised/2018/2018-04/2018-04-22/");

    let conn = Connection::open("db.sqlite").ok().unwrap();
    Image::initialize_db(&conn);
    Subject::initialize_db(&conn);
    
    for entry in fs::read_dir(entry_point).unwrap() {
        let path = entry.ok().unwrap().path();
        if path.extension().unwrap().to_str() == Some("xmp") {
            Image::parse_xmp(&path).insert(&conn);
        }
    }
}
