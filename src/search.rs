//! Database search (Scout-style): ranked full-text-lite search over table
//! columns on any backend. Identifiers are validated before interpolation;
//! the search term always goes through bound parameters.

use crate::orm::Database;
use std::collections::HashMap;

/// One ranked search result row.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: String,
    pub score: i64,
    pub row: HashMap<String, String>,
}

fn valid_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Search `table` for `query` across `columns` (case-insensitive contains),
/// ranked: exact column match > prefix match > substring match, summed per
/// column. Returns strongest hits first, at most `limit`.
pub fn search_database(
    db: &Database,
    table: &str,
    columns: &[&str],
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    if !valid_identifier(table) || columns.is_empty() || !columns.iter().all(|c| valid_identifier(c)) {
        return Err("Invalid search table or column identifier".into());
    }
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let kind = db.kind();
    let ph = kind.placeholder(1);
    let conditions = columns
        .iter()
        .map(|column| format!("LOWER({column}) LIKE {ph}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let column_list = columns.join(", ");
    let sql = format!(
        "SELECT id, {column_list} FROM {table} WHERE {conditions} LIMIT 500"
    );
    let pattern = format!("%{query}%");
    let rows = db.query_params(&sql, &[pattern])?;

    let mut hits: Vec<SearchHit> = rows
        .into_iter()
        .filter_map(|row| {
            let id = row.get("id")?.clone();
            let mut score = 0i64;
            for column in columns {
                if let Some(value) = row.get(*column) {
                    let value = value.to_lowercase();
                    if value == query {
                        score += 100;
                    } else if value.starts_with(&query) {
                        score += 50;
                    } else if value.contains(&query) {
                        score += 10;
                    }
                }
            }
            (score > 0).then_some(SearchHit { id, score, row })
        })
        .collect();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    hits.truncate(limit.max(1));
    Ok(hits)
}
