use super::*;

impl FileIndex {
    pub(super) fn estimate_root_with_budget(
        &self,
        path: &str,
        budget: EstimateBudget,
    ) -> Result<FileIndexRootEstimate> {
        let root = PathBuf::from(path);
        if !root.is_dir() {
            return Err(FileIndexError::InvalidSetting(format!(
                "search scope must be an existing directory: {path}"
            )));
        }
        let options = self.scan_options()?;
        let mut builder = ignore::WalkBuilder::new(&root);
        builder
            .follow_links(false)
            .hidden(!options.include_hidden)
            .git_ignore(options.respect_gitignore)
            .git_global(options.respect_gitignore)
            .git_exclude(options.respect_gitignore)
            .parents(options.respect_gitignore)
            .add_custom_ignore_filename(".rayignore");
        let matcher = estimate_ignore_matcher(&options);

        let started = std::time::Instant::now();
        let mut truncated = false;
        let mut scanned_dirs = 0usize;
        let mut scanned_files = 0usize;
        let mut ignored_or_skipped_files = 0usize;
        let mut indexable_text_files_count = 0usize;
        let mut indexable_text_bytes = 0u64;
        let mut metadata_only_media_files_count = 0usize;
        let mut metadata_only_other_files_count = 0usize;
        let mut estimated_indexed_entries_count = 0usize;
        let mut estimated_index_size_bytes = 0u64;

        for entry in builder.build().filter_map(|entry| entry.ok()) {
            if started.elapsed() >= budget.max_duration
                || scanned_dirs >= budget.max_dirs
                || scanned_files >= budget.max_files
            {
                truncated = true;
                break;
            }
            let path = entry.path();
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                scanned_dirs += 1;
                continue;
            }
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }
            scanned_files += 1;
            if !estimate_should_index_path_for_root(path, &root, &options, matcher.as_ref()) {
                ignored_or_skipped_files += 1;
                continue;
            }

            let file_len = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            let name = entry.file_name().to_string_lossy();
            estimated_indexed_entries_count += 1;
            estimated_index_size_bytes = estimated_index_size_bytes
                .saturating_add(estimate_index_entry_size_bytes(path, &name));

            match classify_indexed_file(path) {
                IndexedFileClass::TextLike => {
                    indexable_text_files_count += 1;
                    indexable_text_bytes = indexable_text_bytes.saturating_add(file_len);
                }
                IndexedFileClass::Media => {
                    metadata_only_media_files_count += 1;
                }
                IndexedFileClass::Other => {
                    metadata_only_other_files_count += 1;
                }
            }
        }

        let mut limitations = vec![concat!(
            "Текущий file index индексирует только имя и путь файла; content indexing не ",
            "используется."
        )
        .to_string()];
        if options.respect_gitignore {
            limitations.push(
                concat!(
                    "Файлы, отфильтрованные .gitignore/ignore walker-ом, считаются не полностью; ",
                    "ignored_or_skipped_files — нижняя оценка."
                )
                .to_string(),
            );
        }
        if truncated {
            limitations.push(
                "Оценка усечена по budget/time cap; итоговые числа являются нижней оценкой."
                    .to_string(),
            );
        }

        let (risk_level, risk_reasons) = assess_estimate_risk(
            path,
            truncated,
            estimated_indexed_entries_count,
            estimated_index_size_bytes,
        );
        Ok(FileIndexRootEstimate {
            path: root.to_string_lossy().to_string(),
            truncated,
            scanned_dirs,
            scanned_files,
            ignored_or_skipped_files,
            indexable_text_files_count,
            indexable_text_bytes,
            metadata_only_media_files_count,
            metadata_only_other_files_count,
            estimated_indexed_entries_count,
            estimated_index_size_bytes,
            risk_level,
            risk_reasons,
            limitations,
        })
    }
}

pub(super) struct EstimateBudget {
    pub(super) max_files: usize,
    pub(super) max_dirs: usize,
    pub(super) max_duration: std::time::Duration,
}

impl Default for EstimateBudget {
    fn default() -> Self {
        Self {
            max_files: 50_000,
            max_dirs: 10_000,
            max_duration: std::time::Duration::from_secs(3),
        }
    }
}
