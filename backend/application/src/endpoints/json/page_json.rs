use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// PD-028: every entity list is paginated. The console renders these with
/// TanStack Table, whose `pageIndex` is **zero-based**, so `page` is too —
/// translating at the boundary is the kind of off-by-one nobody finds until a
/// customer reports a missing row.
pub const DEFAULT_PAGE_SIZE: u64 = 25;
pub const MAX_PAGE_SIZE: u64 = 200;

#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    /// Zero-based page index. Absent or negative is treated as the first page.
    pub page: Option<u64>,
    /// Rows per page. Absent falls back to 25; anything above 200 is clamped,
    /// so a caller cannot ask for the whole table and call it a page.
    pub page_size: Option<u64>,
    /// Free-text filter, applied by the database across that resource's own
    /// searchable columns. It belongs here rather than in the console because
    /// filtering a single fetched page would search 25 rows and silently
    /// ignore the rest.
    pub search: Option<String>,
}

impl PageQuery {
    pub fn page(&self) -> u64 {
        self.page.unwrap_or(0)
    }

    /// `None` when nothing usable was asked for, so callers can skip the
    /// filter entirely rather than matching on an empty string.
    pub fn search(&self) -> Option<String> {
        self.search
            .as_deref()
            .map(str::trim)
            .filter(|term| !term.is_empty())
            .map(str::to_string)
    }

    pub fn page_size(&self) -> u64 {
        match self.page_size {
            Some(0) | None => DEFAULT_PAGE_SIZE,
            Some(requested) => requested.min(MAX_PAGE_SIZE),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageJson<T> {
    pub items: Vec<T>,
    /// Zero-based, matching the request.
    pub page: u64,
    pub page_size: u64,
    pub total_items: u64,
    pub total_pages: u64,
}

impl<T> PageJson<T> {
    pub fn new(items: Vec<T>, page: u64, page_size: u64, total_items: u64) -> Self {
        Self {
            items,
            page,
            page_size,
            total_items,
            total_pages: total_items.div_ceil(page_size.max(1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, PageJson, PageQuery};

    fn query(page: Option<u64>, page_size: Option<u64>) -> PageQuery {
        PageQuery {
            page,
            page_size,
            search: None,
        }
    }

    #[test]
    fn a_blank_search_term_is_no_search_at_all() {
        let blank = PageQuery {
            page: None,
            page_size: None,
            search: Some("   ".to_string()),
        };
        assert_eq!(blank.search(), None);
        let typed = PageQuery {
            page: None,
            page_size: None,
            search: Some("  acme ".to_string()),
        };
        assert_eq!(typed.search().as_deref(), Some("acme"));
    }

    #[test]
    fn defaults_apply_when_nothing_is_asked_for() {
        let q = query(None, None);
        assert_eq!(q.page(), 0);
        assert_eq!(q.page_size(), DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn page_size_is_clamped_rather_than_rejected() {
        assert_eq!(query(None, Some(10_000)).page_size(), MAX_PAGE_SIZE);
        // Zero would divide by zero downstream and means "no page" anyway.
        assert_eq!(query(None, Some(0)).page_size(), DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn total_pages_rounds_up_for_a_partial_last_page() {
        assert_eq!(PageJson::new(vec![1], 0, 25, 51).total_pages, 3);
        assert_eq!(PageJson::new(vec![1], 0, 25, 50).total_pages, 2);
    }

    #[test]
    fn an_empty_result_has_no_pages_and_does_not_divide_by_zero() {
        let empty = PageJson::new(Vec::<u8>::new(), 0, 25, 0);
        assert_eq!(empty.total_pages, 0);
        assert_eq!(PageJson::new(Vec::<u8>::new(), 0, 0, 0).total_pages, 0);
    }
}
