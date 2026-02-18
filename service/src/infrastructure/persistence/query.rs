use luminair_common::persistence::QualifiedTable;
use crate::infrastructure::persistence::parameters::{QueryParameter, QueryParameterRef};

/// High-level, composable query builder
/// Similar to jOOQ, but with Rust's type system
#[derive(Debug)]
pub struct QueryBuilder<'a> {
    from_table: QualifiedTable<'a>,
    select: Vec<ColumnRef<'a>>,
    where_conditions: Vec<Condition<'a>>,
    order_by: Vec<OrderBy<'a>>,
    limit: Option<i64>,
    offset: Option<i64>,
    joins: Vec<Join<'a>>,
}

/// A where condition that will be AND'ed together
#[derive(Debug)]
pub enum Condition<'a> {
    /// field = value
    Equals {
        column: ColumnRef<'a>,
        value: QueryParameterRef,
    },
    
    /// field = ANY(value)
    EqualsAny {
        column: ColumnRef<'a>,
        value: QueryParameterRef,
    },
    
    /// field > value
    GreaterThan {
        column: ColumnRef<'a>,
        value: QueryParameterRef,
    },
    
    /// field < value
    LessThan {
        column: ColumnRef<'a>,
        value: QueryParameterRef,
    },
    
    /// field >= value
    GreaterThanOrEqual {
        column: ColumnRef<'a>,
        value: QueryParameterRef,
    },
    
    /// field <= value
    LessThanOrEqual {
        column: ColumnRef<'a>,
        value: QueryParameterRef,
    },
    
    /// field LIKE '%value%'
    Contains {
        column: ColumnRef<'a>,
        value: QueryParameterRef,
    },
    
    /// field IN (values)
    In {
        column: ColumnRef<'a>,
        values: Vec<QueryParameterRef>,
    },
    
    /// field IS NULL
    IsNull {
        column: ColumnRef<'a>,
    },
    
    /// field IS NOT NULL
    IsNotNull {
        column: ColumnRef<'a>,
    },
    
    /// Combine multiple conditions with OR
    Or(Box<Condition<'a>>, Box<Condition<'a>>),
}

#[derive(Debug)]
pub struct OrderBy<'a> {
    pub column: ColumnRef<'a>,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug)]
pub struct Join<'a> {
    pub join_type: JoinType,
    pub target_table: QualifiedTable<'a>,
    pub main_column: ColumnRef<'a>,
    pub target_column: ColumnRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub enum JoinType {
    Inner,
    Left,
    Right,
}

impl <'a> From<QualifiedTable<'a>> for QueryBuilder<'a> {
    fn from(value: QualifiedTable<'a>) -> Self {
        QueryBuilder {
            from_table: value,
            select: vec![],
            where_conditions: vec![],
            order_by: vec![],
            limit: None,
            offset: None,
            joins: vec![],
        }
    }
}

impl <'a> QueryBuilder<'a> {
    /// Select specified columns
    pub fn select(mut self, columns: Vec<ColumnRef<'a>>) -> Self {
        self.select = columns;
        self
    }

    /// Add where condition
    pub fn where_condition(mut self, condition: Condition<'a>) -> Self {
        self.where_conditions.push(condition);
        self
    }

    /// Add a join clause
    pub fn join(mut self, join: Join<'a>) -> Self {
        self.joins.push(join);
        self
    }

    /// Add an order-by clause
    pub fn order_by(mut self, order: OrderBy<'a>) -> Self {
        self.order_by.push(order);
        self
    }

    /// Build the SQL query string
    pub fn build(self) -> (String, Vec<QueryParameterRef>) {
        let mut sql = String::new();
        let mut params = Vec::new();
        let mut param_counter = 1;
        
        // SELECT clause
        sql.push_str("SELECT ");
        let columns: Vec<String> = self.select.iter()
            .map(|c| c.qualified())
            .collect();
        sql.push_str(&columns.join(", "));

        // FROM clause
        sql.push_str(&format!("\nFROM {}", self.from_table.qualified()));
        
        // JOIN clauses
        for join in &self.joins {
            let join_keyword = match join.join_type {
                JoinType::Inner => "INNER JOIN",
                JoinType::Left => "LEFT JOIN",
                JoinType::Right => "RIGHT JOIN",
            };
            sql.push_str(&format!("\n{} \"{}\" ON {} = {}", join_keyword, join.target_table.qualified(), join.main_column.qualified(), join.target_column.qualified()));
        }

        // WHERE clause
        if !self.where_conditions.is_empty() {
            sql.push_str("\nWHERE ");
            let (where_clause, where_params) = Self::generate_where_conditions(&self.where_conditions, &mut param_counter);
            sql.push_str(&where_clause);
            params.extend(where_params);
        }

        // ORDER BY clause
        if !self.order_by.is_empty() {
            sql.push_str("\nORDER BY ");
            let order_clauses: Vec<String> = self
                .order_by
                .iter()
                .map(|ob| {
                    let direction = match ob.direction {
                        SortDirection::Ascending => "ASC",
                        SortDirection::Descending => "DESC",
                    };
                    format!("{} {}", ob.column.qualified(), direction)
                })
                .collect();
            sql.push_str(&order_clauses.join(", "));
        }
        
        // LIMIT clause
        if let Some(limit) = self.limit {
            sql.push_str(&format!("\nLIMIT {}", limit));
        }
        // OFFSET clause
        if let Some(offset) = self.offset {
            sql.push_str(&format!("\nOFFSET {}", offset));
        }

        (sql, params)
    }

    /// Generate WHERE condition
    fn generate_where_conditions(
        conditions: &[Condition<'a>],
        param_counter: &mut usize,
    ) -> (String, Vec<QueryParameterRef>) {
        let mut where_sql = Vec::new();
        let mut params = Vec::new();
        
        for condition in conditions {
            let (cond_sql, cond_params) = condition.to_sql(param_counter);
            where_sql.push(cond_sql);
            params.extend(cond_params);
        }
        
        (where_sql.join(" AND "), params)
    }

}

impl <'a> Condition<'a> {
    pub fn to_sql(&self, param_counter: &mut usize) -> (String, Vec<QueryParameterRef>) {
        match self {
            Condition::Equals { column, value } => {
                let sql = format!("{} = ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![*value])
            }
            
            Condition::GreaterThan { column, value } => {
                let sql = format!("{} > ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![*value])
            }
            
            Condition::LessThan { column, value } => {
                let sql = format!("{} < ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![*value])
            }
            
            Condition::GreaterThanOrEqual { column, value } => {
                let sql = format!("{} >= ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![*value])
            }
            
            Condition::LessThanOrEqual { column, value } => {
                let sql = format!("{} <= ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![*value])
            }
            
            Condition::Contains { column, value } => {
                let sql = format!("{} ILIKE ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![*value])
            }
            
            Condition::In { column, values } => {
                let placeholders: Vec<String> = values
                    .iter()
                    .map(|_| {
                        let placeholder = format!("${}", param_counter);
                        *param_counter += 1;
                        placeholder
                    })
                    .collect();
                
                let sql = format!("{} IN ({})", column.qualified(), placeholders.join(", "));
                let params: Vec<QueryParameterRef> = values
                    .iter()
                    .map(|v| *v)
                    .collect();
                (sql, params)
            }
            
            Condition::IsNull { column } => {
                let sql = format!("{} IS NULL", column.qualified());
                (sql, vec![])
            }
            
            Condition::IsNotNull { column } => {
                let sql = format!("{} IS NOT NULL", column.qualified());
                (sql, vec![])
            }
            
            Condition::Or(left, right) => {
                let (left_sql, mut left_params) = left.to_sql(param_counter);
                let (right_sql, right_params) = right.to_sql(param_counter);
                let sql = format!("({} OR {})", left_sql, right_sql);
                left_params.extend(right_params);
                (sql, left_params)
            }
            
            Condition::EqualsAny { column, value } => {
                let sql = format!("{} = ANY ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![*value])
            }
        }
    }
}

use std::borrow::Cow;

/// Represents one column in the database table
#[derive(Clone, Debug)]
pub struct Column<'a> {
    pub qualifier: &'static str,
    pub name: Cow<'a, str>,
}

impl <'a> Column<'a> {
    /// Get qualified column name
    pub fn qualified(&self) -> String {
        format!("\"{}\".\"{}\"", self.qualifier, self.name)
    }
}

/// Column reference which can be either borrowed or owned
pub type ColumnRef<'a> = Cow<'a, Column<'a>>;
