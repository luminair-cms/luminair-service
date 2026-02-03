use luminair_common::DocumentTypeId;

use crate::domain::document::content::DomainValue;

/// Query for finding DocumentInstances
#[derive(Debug, Clone)]
pub struct DocumentInstanceQuery {
    /// Which DocumentType are we querying?
    pub document_type_id: DocumentTypeId,
    
    pub filter: FilterExpression,
    pub sort: Vec<(String, SortDirection)>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    
    /// Include draft instances?
    pub include_drafts: bool,
}

/// Filter expressions for querying documents
#[derive(Debug, Clone)]
pub enum FilterExpression {
    /// No filter - all documents
    None,
    
    /// Exact match: field = value
    Equals { field: String, value: DomainValue },
    
    /// Greater than
    GreaterThan { field: String, value: i64 },
    
    /// Less than
    LessThan { field: String, value: i64 },
    
    /// Contains (for text fields)
    Contains { field: String, value: String },
    
    /// For relations: document has related document
    HasRelation { field: String, id: DocumentTypeId },
    
    /// Combine filters with AND
    And(Box<FilterExpression>, Box<FilterExpression>),
    
    /// Combine filters with OR
    Or(Box<FilterExpression>, Box<FilterExpression>),
}

#[derive(Debug, Clone)]
pub enum SortDirection {
    Ascending,
    Descending,
}