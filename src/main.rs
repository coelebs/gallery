#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate quick_xml;
extern crate rusqlite;
extern crate rocket;

use quick_xml::reader::Reader;
use quick_xml::events::Event;
use quick_xml::events::attributes::Attribute;

use rusqlite::Connection;

use std::io::BufReader;
use std::fs;
use std::path;
use std::env;

#[derive(Debug)]
struct Image {
    id: i64,
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

        conn.execute("DROP TABLE IF EXISTS Image;", &[]).unwrap();
        conn.execute("CREATE TABLE Image (
                        id          INTEGER PRIMARY KEY,
                        path        TEXT,
                        rating      INTEGER
                      );", &[]).unwrap();

        conn.execute("DROP TABLE IF EXISTS Image_Subjects;", &[]).unwrap();
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

        Image {id: -1, path: path.to_path_buf(), rating: rating.unwrap(), subjects: Subject::parse_subjects(&subjects)}
    }
}

impl Subject {
    fn initialize_db(conn: &Connection) {
        conn.execute("DROP TABLE IF EXISTS Subject;", &[]).unwrap();
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

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/gallery/<rating>")]
fn gallery(rating: u8) -> String {
    let mut result = String::new();

    let conn = Connection::open("db.sqlite").ok().unwrap();
    let mut qry = conn.prepare("SELECT id, rating, path FROM Image WHERE rating >= ?1").unwrap();

    let image_iter = qry.query_map(&[&rating], |row| {
         let path_s: String = row.get(2);
         Image {
            id: row.get(0),
            path: path::PathBuf::from(path_s),
            rating: row.get(1),
            subjects: Vec::new()
        }
    }).unwrap();
    
    let mut subj_qry = conn.prepare("SELECT s.id, s.family, person FROM Subject s
                            LEFT JOIN Image_Subjects ims ON ims.subject_id = s.id
                            WHERE ims.image_id = ?1").unwrap();
    for i in image_iter {
        let image = i.unwrap();
        result.push_str(&format!("{:?}: {:?}\n", image.rating, image.path));

        let subject_iter = subj_qry.query_map(&[&image.id], |row| {
            Subject {
                id: row.get(0),
                family: row.get(1),
                person: row.get(2),
            }
        }).unwrap();

        for s in subject_iter {
            let subject = s.unwrap();
            result.push_str(&format!("\t\t{:?}|{:?}\n", subject.family, subject.person));
        }

        result.push('\n');
    }

    result
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let entry_point = path::Path::new("/mnt/freenas/pictures/organised/2018/2018-04/2018-04-22/");


    if args.len() > 1 {
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

    rocket::ignite().mount("/", routes![index, gallery]).launch();
}
