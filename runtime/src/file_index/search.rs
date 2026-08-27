use super::*;

impl FileIndex {
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FileSearchResult>> {
        if !self.enabled || !self.index_enabled()? {
            return Ok(Vec::new());
        }
        self.observe_search(query);
        let candidate_limit = limit.saturating_mul(64).max(512);
        let roots = self.root_strings()?;
        let candidates = self
            .store
            .search(query, candidate_limit)?
            .into_iter()
            .filter(|file| indexed_result_is_visible(&file.path, &roots))
            .collect();
        Ok(rank(candidates, query, limit))
    }

    pub fn open(&self, path: &str) -> Result<()> {
        let target = std::fs::canonicalize(path)
            .map_err(|_| FileIndexError::MissingFile(path.to_string()))?;
        if !target.is_file() {
            return Err(FileIndexError::MissingFile(path.to_string()));
        }
        let roots = self
            .root_paths()?
            .into_iter()
            .filter_map(|root| std::fs::canonicalize(root).ok())
            .map(|root| root.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        if !indexed_result_is_visible(&target.to_string_lossy(), &roots) {
            return Err(FileIndexError::MissingFile(path.to_string()));
        }
        scanner::open_file(&target.to_string_lossy())
    }
}

fn rank(files: Vec<IndexedFile>, query: &str, limit: usize) -> Vec<FileSearchResult> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<FileSearchResult> = files
        .into_iter()
        .filter_map(|file| {
            let display_name = display_name(&file);
            let name = display_name.to_lowercase();
            let path = file.path.to_lowercase();
            let score = if name.starts_with(&q) {
                1.0
            } else if word_prefix(&name, &q) {
                0.82
            } else if name.contains(&q) {
                0.68
            } else if path.contains(&q) {
                0.35
            } else {
                return None;
            };
            Some(FileSearchResult {
                path: file.path,
                name: display_name,
                score,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });
    out.truncate(limit);
    out
}

fn display_name(file: &IndexedFile) -> String {
    let stored = file.name.trim();
    if !stored.is_empty() {
        return stored.to_string();
    }
    file.path
        .rsplit(['\\', '/'])
        .find(|part| !part.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Без названия".to_string())
}

fn word_prefix(name: &str, query: &str) -> bool {
    [' ', '-', '_', '.', '/', '\\']
        .iter()
        .any(|sep| name.split(*sep).any(|part| part.starts_with(query)))
}
