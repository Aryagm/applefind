pub mod content;
pub mod dataset;
pub mod index;
pub mod query;

pub use content::{ContentIndex, ContentMatch, ContentSearchResult, ContentSearchStats};
pub use index::{PathIndex, SearchHit, SearchMode, SearchResult, SearchStats};
pub use query::{ParsedQuery, QueryToken, SearchField, parse_query};
