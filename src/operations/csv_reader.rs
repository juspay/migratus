use crate::domain::{records::Record, types::LineNumber};
use crate::error::*;
use std::collections::HashMap;
use std::path::Path;

pub struct CsvReader {
    headers: Vec<String>,
}

impl CsvReader {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
        }
    }

    pub fn get_headers(&self) -> Vec<String> {
        self.headers.clone()
    }

    pub fn read_file(&mut self, path: &Path) -> Result<Vec<Record>> {
        let mut reader = csv::Reader::from_path(path)?;

        self.headers = reader.headers()?.iter().map(|h| h.to_string()).collect();

        let mut records = Vec::new();
        let mut line_num = 1;

        for result in reader.records() {
            let csv_record = result?;
            let mut data = HashMap::new();

            for (i, field) in csv_record.iter().enumerate() {
                if let Some(header) = self.headers.get(i) {
                    data.insert(header.clone(), field.to_string());
                }
            }

            records.push(Record::new(LineNumber::new(line_num), data));
            line_num += 1;
        }

        Ok(records)
    }

    pub fn headers(&self) -> &[String] {
        &self.headers
    }
}

impl Default for CsvReader {
    fn default() -> Self {
        Self::new()
    }
}
