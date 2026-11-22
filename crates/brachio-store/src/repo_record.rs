//! Registered projects: git repositories and plain folders.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

use crate::database::StoreError;

/// A repository the user added to Brachio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRecord {
    /// Stable id.
    pub id: String,
    /// Display name, by default the folder name.
    pub name: String,
    /// Absolute path of the repository root.
    pub path: PathBuf,
    /// Branch new tasks start from, for example `main`. Empty for a plain folder.
    pub default_branch: String,
    /// When the repository was added.
    pub added_at_ms: i64,
    /// False for a plain folder: its tasks run in the folder itself, with no branch or worktree.
    pub is_git: bool,
}

/// Fields for a new repository row.
#[derive(Debug, Clone)]
pub struct NewRepo<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Display name.
    pub name: &'a str,
    /// Absolute path of the repository root.
    pub path: &'a Path,
    /// Branch new tasks start from.
    pub default_branch: &'a str,
    /// Creation time.
    pub added_at_ms: i64,
    /// False for a plain folder.
    pub is_git: bool,
}

const COLUMNS: &str = "id, name, path, default_branch, added_at_ms, is_git";

impl RepoRecord {
    /// Adds a repository.
    ///
    /// # Errors
    /// Fails when the path or id is already registered.
    pub fn insert(connection: &Connection, repo: &NewRepo<'_>) -> Result<Self, StoreError> {
        connection.execute(
            "INSERT INTO repos (id, name, path, default_branch, added_at_ms, is_git)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                repo.id,
                repo.name,
                repo.path.to_string_lossy(),
                repo.default_branch,
                repo.added_at_ms,
                repo.is_git
            ],
        )?;
        Self::get(connection, repo.id)
    }

    /// All repositories in the order the user placed them, then the rest oldest first.
    ///
    /// # Errors
    /// Fails when the query fails.
    pub fn list(connection: &Connection) -> Result<Vec<Self>, StoreError> {
        let sql = format!("SELECT {COLUMNS} FROM repos ORDER BY sort_order IS NULL, sort_order, added_at_ms, name");
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], Self::from_row)?;
        let repos: Vec<Self> = rows.collect::<Result<_, _>>()?;
        Ok(repos)
    }

    /// Places the repositories in the given order. Repositories left out keep sorting after them.
    ///
    /// # Errors
    /// Fails with `NotFound` for an unknown id, and changes nothing.
    pub fn reorder(connection: &mut Connection, ids: &[String]) -> Result<(), StoreError> {
        let transaction = connection.transaction()?;
        for (position, id) in ids.iter().enumerate() {
            let changed = transaction.execute(
                "UPDATE repos SET sort_order = ?2 WHERE id = ?1",
                params![id, i64::try_from(position).unwrap_or(i64::MAX)],
            )?;
            if changed == 0 {
                return Err(StoreError::NotFound {
                    table: "repos",
                    id: id.clone(),
                });
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// One repository by id.
    ///
    /// # Errors
    /// Fails with `NotFound` when no repository has this id.
    pub fn get(connection: &Connection, id: &str) -> Result<Self, StoreError> {
        let sql = format!("SELECT {COLUMNS} FROM repos WHERE id = ?1");
        let repo = connection
            .query_row(&sql, [id], Self::from_row)
            .optional()?;
        repo.ok_or_else(|| StoreError::NotFound {
            table: "repos",
            id: id.to_owned(),
        })
    }

    /// The repository registered at `path`, if any.
    ///
    /// # Errors
    /// Fails when the query fails.
    pub fn find_by_path(connection: &Connection, path: &Path) -> Result<Option<Self>, StoreError> {
        let sql = format!("SELECT {COLUMNS} FROM repos WHERE path = ?1");
        let path = path.to_string_lossy();
        let repo = connection
            .query_row(&sql, [path], Self::from_row)
            .optional()?;
        Ok(repo)
    }

    /// Removes a repository. Its tasks must be removed first.
    ///
    /// # Errors
    /// Fails when tasks still point at the repository.
    pub fn delete(connection: &Connection, id: &str) -> Result<(), StoreError> {
        connection.execute("DELETE FROM repos WHERE id = ?1", [id])?;
        Ok(())
    }

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let path: String = row.get(2)?;
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            path: PathBuf::from(path),
            default_branch: row.get(3)?,
            added_at_ms: row.get(4)?,
            is_git: row.get(5)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;

    fn sample<'a>(id: &'a str, path: &'a Path) -> NewRepo<'a> {
        NewRepo {
            id,
            name: "brachio",
            path,
            default_branch: "main",
            added_at_ms: 1,
            is_git: true,
        }
    }

    #[test]
    fn insert_then_find_by_path() {
        let connection = database::open_in_memory().expect("open database");
        let path = Path::new("/src/brachio");

        let inserted = RepoRecord::insert(&connection, &sample("r1", path)).expect("insert");

        let found = RepoRecord::find_by_path(&connection, path).expect("query");
        assert_eq!(found, Some(inserted));
        assert_eq!(RepoRecord::list(&connection).expect("list").len(), 1);
    }

    #[test]
    fn same_path_cannot_be_added_twice() {
        let connection = database::open_in_memory().expect("open database");
        let path = Path::new("/src/brachio");
        RepoRecord::insert(&connection, &sample("r1", path)).expect("first insert");

        let second = RepoRecord::insert(&connection, &sample("r2", path));

        assert!(second.is_err(), "the path column is unique");
    }

    #[test]
    fn missing_id_is_not_found() {
        let connection = database::open_in_memory().expect("open database");
        let missing = RepoRecord::get(&connection, "nope");
        assert!(
            matches!(missing, Err(StoreError::NotFound { .. })),
            "{missing:?}"
        );
    }
}
