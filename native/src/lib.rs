use analysis::Analysis;
use filetime::FileTime;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub struct FileInfo {
    pub created_at: SystemTime,
    pub name: String,
    pub path: String,
}

pub fn run_analyzer(demo_path: &PathBuf) -> (FileInfo, Analysis) {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .open(demo_path)
        .expect("Could not open the file");

    let mut bytes: Vec<u8> = vec![];

    file.read_to_end(&mut bytes)
        .expect("Could not read the file");

    let analysis = Analysis::from(bytes.as_slice());

    let created_at = fs::metadata(demo_path)
        .map_err(|_| ())
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .map(|file_time| {
            let creation_offset =
                Duration::new(file_time.unix_seconds() as u64, file_time.nanoseconds());

            SystemTime::UNIX_EPOCH + creation_offset
        })
        .unwrap();

    let file_info = FileInfo {
        created_at,
        name: demo_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(String::from)
            .unwrap(),

        path: demo_path.to_str().map(String::from).unwrap(),
    };

    (file_info, analysis)
}
