use std::path::{Component, Path, PathBuf};

pub fn assert_existing_dir(path: &Path) -> Result<(), crate::error::Error> {
    if !path.exists() {
        return Err(crate::error::Error::NotDirectory(path.to_path_buf()));
    }
    if !path.is_dir() {
        return Err(crate::error::Error::NotDirectory(path.to_path_buf()));
    }
    Ok(())
}

pub fn assert_existing_file(path: &Path) -> Result<(), crate::error::Error> {
    if !path.exists() {
        return Err(crate::error::Error::PlanNotFound(path.to_path_buf()));
    }
    if !path.is_file() {
        return Err(crate::error::Error::NotRegularFile(path.to_path_buf()));
    }
    Ok(())
}

pub fn plan_is_inside_repo(plan_path: &Path, repo_path: &Path) -> bool {
    plan_path.starts_with(repo_path) && plan_path != repo_path
}

pub fn relative_plan_path(plan_path: &Path, repo_path: &Path) -> PathBuf {
    plan_path
        .strip_prefix(repo_path)
        .unwrap_or(plan_path)
        .to_path_buf()
}

pub fn validate_gov_dir_exists(repo: &Path) -> Result<PathBuf, crate::error::Error> {
    let mrgs_path = repo.join(".mrgs");
    let metadata = match std::fs::symlink_metadata(&mrgs_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(crate::error::Error::GovDirNotExists(mrgs_path));
        }
        Err(error) => return Err(error.into()),
    };

    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(crate::error::Error::GovDirEscape(mrgs_path));
    }
    if !metadata.is_dir() {
        return Err(crate::error::Error::GovDirNotDirectory(mrgs_path));
    }

    let canonical_repo = std::fs::canonicalize(repo)?;
    let expected = canonical_repo.join(".mrgs");
    let canonical = std::fs::canonicalize(&mrgs_path)?;
    if canonical != expected {
        return Err(crate::error::Error::GovDirEscape(mrgs_path));
    }
    Ok(canonical)
}

pub fn validate_gov_dir(repo: &Path) -> Result<PathBuf, crate::error::Error> {
    let mrgs_path = repo.join(".mrgs");
    match std::fs::symlink_metadata(&mrgs_path) {
        Ok(_) => validate_gov_dir_exists(repo),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(&mrgs_path)?;
            validate_gov_dir_exists(repo)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

pub fn validate_safe_relative_path(path_str: &str) -> Result<PathBuf, crate::error::Error> {
    if path_str.is_empty() {
        return Err(crate::error::Error::EmptyPlanPath);
    }

    let path = Path::new(path_str);

    #[cfg(windows)]
    {
        if path_str.len() >= 2
            && path_str.as_bytes()[1] == b':'
            && path_str
                .as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_alphabetic())
        {
            return Err(crate::error::Error::UnsafePlanPath(path_str.to_string()));
        }
    }

    for component in path.components() {
        match component {
            Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_)
            | Component::CurDir => {
                return Err(crate::error::Error::UnsafePlanPath(path_str.to_string()));
            }
            Component::Normal(_) => {}
        }
    }

    Ok(path.to_path_buf())
}

pub fn resolve_safe_plan_path(
    repo: &Path,
    plan_path_str: &str,
) -> Result<PathBuf, crate::error::Error> {
    let relative = validate_safe_relative_path(plan_path_str)?;
    let candidate = repo.join(&relative);

    let canonical = std::fs::canonicalize(&candidate)?;

    if !plan_is_inside_repo(&canonical, repo) {
        return Err(crate::error::Error::PlanPathOutsideRepo);
    }

    assert_existing_file(&canonical)?;

    Ok(canonical)
}
