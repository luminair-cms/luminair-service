use luminair_common::DocumentTypeId;

use crate::domain::document::content::DomainValue;

/// Represents the publication status filter for document queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentStatus {
    /// Include only published documents
    Published,
    /// Include only draft documents (with published instances)
    Draft,
}

impl Default for DocumentStatus {
    fn default() -> Self {
        DocumentStatus::Published
    }
}

impl DocumentStatus {
    /// Convert status to include_drafts flag
    pub fn to_include_drafts(self) -> bool {
        matches!(self, DocumentStatus::Draft)
    }
}

/// Query for finding DocumentInstances
#[derive(Debug, Clone)]
pub struct DocumentInstanceQuery {
    pub filter: FilterExpression,
    pub sort: Vec<Sort>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,

    /// Include draft instances?
    pub include_drafts: bool,
}

impl DocumentInstanceQuery {
    /// Create a new query builder for the given document type
    pub fn new() -> Self {
        Self {
            filter: FilterExpression::None,
            sort: Vec::new(),
            limit: None,
            offset: None,
            include_drafts: false,
        }
    }

    /// Set the filter expression
    pub fn with_filter(mut self, filter: FilterExpression) -> Self {
        self.filter = filter;
        self
    }

    /// Add equality filter: field = value
    pub fn filter_equals(self, field: String, value: DomainValue) -> Self {
        self.with_filter(FilterExpression::Equals { field, value })
    }

    /// Add inequality filter: field != value
    pub fn filter_not_equals(self, field: String, value: DomainValue) -> Self {
        self.with_filter(FilterExpression::NotEquals { field, value })
    }

    /// Add greater than filter: field > value
    pub fn filter_greater_than(self, field: String, value: DomainValue) -> Self {
        self.with_filter(FilterExpression::GreaterThan { field, value })
    }

    /// Add greater than or equal filter: field >= value
    pub fn filter_greater_than_or_equal(self, field: String, value: DomainValue) -> Self {
        self.with_filter(FilterExpression::GreaterThanOrEqual { field, value })
    }

    /// Add less than filter: field < value
    pub fn filter_less_than(self, field: String, value: DomainValue) -> Self {
        self.with_filter(FilterExpression::LessThan { field, value })
    }

    /// Add less than or equal filter: field <= value
    pub fn filter_less_than_or_equal(self, field: String, value: DomainValue) -> Self {
        self.with_filter(FilterExpression::LessThanOrEqual { field, value })
    }

    /// Add in filter: field in (values)
    pub fn filter_in(self, field: String, values: Vec<DomainValue>) -> Self {
        self.with_filter(FilterExpression::In { field, values })
    }

    /// Add not in filter: field not in (values)
    pub fn filter_not_in(self, field: String, values: Vec<DomainValue>) -> Self {
        self.with_filter(FilterExpression::NotIn { field, values })
    }

    /// Add contains filter: field contains value (for text fields)
    pub fn filter_contains(self, field: String, value: String) -> Self {
        self.with_filter(FilterExpression::Contains { field, value })
    }

    /// Add starts with filter: field starts with value
    pub fn filter_starts_with(self, field: String, value: String) -> Self {
        self.with_filter(FilterExpression::StartsWith { field, value })
    }

    /// Add ends with filter: field ends with value
    pub fn filter_ends_with(self, field: String, value: String) -> Self {
        self.with_filter(FilterExpression::EndsWith { field, value })
    }

    /// Add is null filter
    pub fn filter_is_null(self, field: String) -> Self {
        self.with_filter(FilterExpression::IsNull { field })
    }

    /// Add is not null filter
    pub fn filter_is_not_null(self, field: String) -> Self {
        self.with_filter(FilterExpression::IsNotNull { field })
    }

    /// Add relation filter: document has related document
    pub fn filter_has_relation(self, field: String, id: DocumentTypeId) -> Self {
        self.with_filter(FilterExpression::HasRelation { field, id })
    }

    /// Combine current filter with AND operator
    pub fn and(mut self, other: FilterExpression) -> Self {
        let current = std::mem::replace(&mut self.filter, FilterExpression::None);
        self.filter = match current {
            FilterExpression::None => other,
            _ => FilterExpression::And(Box::new(current), Box::new(other)),
        };
        self
    }

    /// Combine current filter with OR operator
    pub fn or(mut self, other: FilterExpression) -> Self {
        let current = std::mem::replace(&mut self.filter, FilterExpression::None);
        self.filter = match current {
            FilterExpression::None => other,
            _ => FilterExpression::Or(Box::new(current), Box::new(other)),
        };
        self
    }

    /// Add sort order
    pub fn add_sort(mut self, field: String, direction: SortDirection) -> Self {
        self.sort.push(Sort { field, direction });
        self
    }

    /// Set pagination limit
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set pagination offset
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set pagination using page number and page size
    ///
    /// Enforces invariants:
    /// - Page defaults to 1 if 0
    /// - Page size is capped at 200
    pub fn paginate(mut self, mut page: u16, mut page_size: u16) -> Self {
        // Ensure page is at least 1
        if page == 0 {
            page = 1;
        }

        // Ensure page_size doesn't exceed 200
        if page_size > 200 {
            page_size = 200;
        }

        if page > 0 && page_size > 0 {
            let offset = ((page - 1) as i64) * (page_size as i64);
            self.offset = Some(offset);
            self.limit = Some(page_size as i64);
        }
        self
    }

    /// Include draft instances in results
    pub fn include_drafts(mut self) -> Self {
        self.include_drafts = true;
        self
    }

    /// Exclude draft instances in results (default)
    pub fn exclude_drafts(mut self) -> Self {
        self.include_drafts = false;
        self
    }

    /// Set publication status filter
    pub fn with_status(mut self, status: DocumentStatus) -> Self {
        self.include_drafts = status.to_include_drafts();
        self
    }
}

/// Filter expressions for querying documents
#[derive(Debug, Clone)]
pub enum FilterExpression {
    /// No filter - all documents
    None,

    /// Exact match: field = value
    Equals { field: String, value: DomainValue },

    /// Not equal match: field != value
    NotEquals { field: String, value: DomainValue },

    /// Greater than
    GreaterThan { field: String, value: DomainValue },

    /// Greater than or equal
    GreaterThanOrEqual { field: String, value: DomainValue },

    /// Less than
    LessThan { field: String, value: DomainValue },

    /// Less than or equal
    LessThanOrEqual { field: String, value: DomainValue },

    /// In: field in (values)
    In {
        field: String,
        values: Vec<DomainValue>,
    },

    /// Not In: field not in (values)
    NotIn {
        field: String,
        values: Vec<DomainValue>,
    },

    /// Contains (for text fields)
    Contains { field: String, value: String },

    /// Starts with (for text fields)
    StartsWith { field: String, value: String },

    /// Ends with (for text fields)
    EndsWith { field: String, value: String },

    /// Is null
    IsNull { field: String },

    /// Is not null
    IsNotNull { field: String },

    /// For relations: document has related document
    HasRelation { field: String, id: DocumentTypeId },

    /// Combine filters with AND
    And(Box<FilterExpression>, Box<FilterExpression>),

    /// Combine filters with OR
    Or(Box<FilterExpression>, Box<FilterExpression>),
}

#[derive(Debug, Clone)]
pub struct Sort {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}
