use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

pub(crate) struct Dict {
    to_id: HashMap<String, u32>,
    values: Vec<String>,
    posts: Vec<Vec<u32>>,
}

impl Dict {
    pub(crate) fn new() -> Self {
        Self {
            to_id: HashMap::new(),
            values: Vec::new(),
            posts: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, value: &str, line: u32) {
        let id = if let Some(&id) = self.to_id.get(value) {
            id
        } else {
            let id = self.values.len() as u32;
            self.to_id.insert(value.to_string(), id);
            self.values.push(value.to_string());
            self.posts.push(Vec::new());
            id
        };
        self.posts[id as usize].push(line);
    }

    pub(crate) fn write(&self, out: &Path, name: &str) -> io::Result<()> {
        let listed = serde_json::to_vec_pretty(&self.values).map_err(io::Error::other)?;
        fs::write(out.join(format!("{name}.json")), listed)?;
        let mut toc = Vec::new();
        let mut posted = Vec::new();
        let mut start = 0u32;
        for bucket in &self.posts {
            toc.extend_from_slice(&start.to_le_bytes());
            toc.extend_from_slice(&(bucket.len() as u32).to_le_bytes());
            for line in bucket {
                posted.extend_from_slice(&line.to_le_bytes());
            }
            start += bucket.len() as u32;
        }
        fs::write(out.join(format!("{name}.toc")), toc)?;
        fs::write(out.join(format!("{name}.ids")), posted)?;
        Ok(())
    }
}
