use plinky_macros::{Display, Error};
use std::collections::BTreeMap;
use std::env::VarError;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn discover(env: &PkgConfigEnv) -> Result<BTreeMap<String, PathBuf>, DiscoverError> {
    let mut found = BTreeMap::new();

    for directory in search_path(env) {
        let dir_reader = match std::fs::read_dir(directory) {
            Ok(reader) => reader,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(DiscoverError::ListDir(directory.into(), err)),
        };
        for entry in dir_reader {
            let entry = entry.map_err(|e| DiscoverError::ListDir(directory.into(), e))?;

            let ty = entry.file_type().map_err(|e| DiscoverError::ReadMetadata(entry.path(), e))?;
            if !ty.is_file() {
                continue;
            }

            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| DiscoverError::NonUtf8FileName(name.clone(), directory.into()))?;

            match name.strip_suffix(".pc") {
                Some(library) => {
                    if !found.contains_key(library) {
                        found.insert(library.into(), directory.join(name));
                    }
                }
                None => continue,
            }
        }
    }

    Ok(found)
}

fn search_path(env: &PkgConfigEnv) -> impl Iterator<Item = &Path> {
    // TODO: add system search path to `paths`.
    [&env.pkg_config_path, &env.pkg_config_path_for_target]
        .into_iter()
        .flat_map(|s| s.as_deref()) // Option<String> to &str when Some()
        .flat_map(|s| s.split(':'))
        .map(Path::new)
}

pub struct PkgConfigEnv {
    pub pkg_config_path: Option<String>,
    pub pkg_config_path_for_target: Option<String>,
}

impl PkgConfigEnv {
    pub fn from_env() -> Result<Self, DiscoverError> {
        let get = |name| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(VarError::NotPresent) => Ok(None),
            Err(VarError::NotUnicode(_)) => Err(DiscoverError::NotUtf8Env(name)),
        };
        Ok(Self {
            pkg_config_path: get("PKG_CONFIG_PATH")?,
            pkg_config_path_for_target: get("PKG_CONFIG_PATH_FOR_TARGET")?,
        })
    }
}

#[derive(Debug, Error, Display)]
pub enum DiscoverError {
    #[display("environment variable {f0} is not UTF-8")]
    NotUtf8Env(&'static str),
    #[display("failed to list directory {f0:?}")]
    ListDir(PathBuf, #[source] std::io::Error),
    #[display("failed to read metadata of {f0:?}")]
    ReadMetadata(PathBuf, #[source] std::io::Error),
    #[display("file name {f0:?} in {f1:?} is not UTF-8")]
    NonUtf8FileName(OsString, PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use plinky_utils::create_temp_dir;

    #[test]
    fn test_discover() {
        let create = |path: &Path| {
            if !path.parent().unwrap().exists() {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            }
            std::fs::write(path, b"").unwrap();
        };

        let root = create_temp_dir().unwrap();
        create(&root.join("foo").join("lib1.pc"));
        create(&root.join("foo").join("lib2.pc"));
        create(&root.join("bar").join("lib1.pc"));

        let env = PkgConfigEnv {
            pkg_config_path: Some(root.join("foo").to_str().unwrap().to_string() + ":missing/dir"),
            pkg_config_path_for_target: Some(root.join("bar").to_str().unwrap().to_string()),
        };
        let discovered = discover(&env).unwrap();

        // lib1 is present in both directories. Respect the precedence.
        assert_eq!(2, discovered.len());
        assert_eq!(root.join("foo").join("lib1.pc"), discovered["lib1"]);
        assert_eq!(root.join("foo").join("lib2.pc"), discovered["lib2"]);
    }

    #[test]
    fn test_search_paths_empty_env() {
        let env = PkgConfigEnv { pkg_config_path: None, pkg_config_path_for_target: None };
        assert!(search_path(&env).next().is_none());
    }

    #[test]
    fn test_search_paths_single_element() {
        let env = PkgConfigEnv {
            pkg_config_path: Some("foo/bar".into()),
            pkg_config_path_for_target: None,
        };

        assert_eq!(vec![Path::new("foo/bar")], search_path(&env).collect::<Vec<_>>());
    }

    #[test]
    fn test_search_paths_multiple_elements() {
        let env = PkgConfigEnv {
            pkg_config_path: Some("foo/bar:baz".into()),
            pkg_config_path_for_target: Some("hello:".into()),
        };

        assert_eq!(
            vec![Path::new("foo/bar"), Path::new("baz"), Path::new("hello"), Path::new("")],
            search_path(&env).collect::<Vec<_>>()
        );
    }
}
