//! Lists as Linear pages them, and references by id.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Connection<T> {
    pub nodes: Vec<T>,
    #[serde(default, rename = "pageInfo")]
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PageInfo {
    #[serde(default, rename = "hasNextPage")]
    pub has_next_page: bool,
    #[serde(default, rename = "endCursor")]
    pub end_cursor: Option<String>,
}

/// Just the id of a related record, for parent/child links.
/// A reference to another entity by id alone, typed by what it points at.
#[derive(Debug, Clone, Deserialize)]
pub struct Ref<I> {
    pub id: I,
}

/// One page of a cursor-paginated list.
#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page_info: PageInfo,
    /// True when this page extends the existing list rather than replacing it.
    pub append: bool,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, page_info: PageInfo, append: bool) -> Self {
        Self {
            items,
            page_info,
            append,
        }
    }
}
