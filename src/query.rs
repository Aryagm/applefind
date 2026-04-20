#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchField {
    BasenameOrPath,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryToken {
    pub text: String,
    pub field: SearchField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    pub raw: String,
    pub tokens: Vec<QueryToken>,
}

pub fn parse_query(raw: &str) -> ParsedQuery {
    let tokens = raw
        .split_whitespace()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| QueryToken {
            text: part.to_lowercase(),
            field: if part.contains('/') || part.contains('\\') {
                SearchField::Path
            } else {
                SearchField::BasenameOrPath
            },
        })
        .collect();

    ParsedQuery {
        raw: raw.to_string(),
        tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::{SearchField, parse_query};

    #[test]
    fn parses_path_and_basename_tokens() {
        let parsed = parse_query("src/lib controller");
        assert_eq!(parsed.tokens.len(), 2);
        assert_eq!(parsed.tokens[0].field, SearchField::Path);
        assert_eq!(parsed.tokens[1].field, SearchField::BasenameOrPath);
        assert_eq!(parsed.tokens[0].text, "src/lib");
        assert_eq!(parsed.tokens[1].text, "controller");
    }
}
