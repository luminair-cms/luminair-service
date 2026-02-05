use crate::infrastructure::persistence::schema::{ColumnRef, Table};

/// High-level, composable query builder
/// Similar to jOOQ, but with Rust's type system
#[derive(Debug, Clone)]
pub struct QueryBuilder<'a> {
    from_table: Table<'a>,
    select: Vec<ColumnRef<'a>>,
    where_conditions: Vec<Condition<'a>>,
    order_by: Vec<OrderBy<'a>>,
    limit: Option<i64>,
    offset: Option<i64>,
    joins: Vec<Join<'a>>,
}

/// A where condition that will be AND'ed together
#[derive(Debug, Clone)]
pub enum Condition<'a> {
    /// field = value
    Equals {
        column: ColumnRef<'a>,
        value: ConditionValue,
    },
    
    /// field > value
    GreaterThan {
        column: ColumnRef<'a>,
        value: i64,
    },
    
    /// field < value
    LessThan {
        column: ColumnRef<'a>,
        value: i64,
    },
    
    /// field >= value
    GreaterThanOrEqual {
        column: ColumnRef<'a>,
        value: i64,
    },
    
    /// field <= value
    LessThanOrEqual {
        column: ColumnRef<'a>,
        value: i64,
    },
    
    /// field LIKE '%value%'
    Contains {
        column: ColumnRef<'a>,
        value: String,
    },
    
    /// field IN (values)
    In {
        column: ColumnRef<'a>,
        values: Vec<ConditionValue>,
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

#[derive(Debug, Clone)]
pub enum ConditionValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone)]
pub struct OrderBy<'a> {
    pub column: ColumnRef<'a>,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
pub struct Join<'a> {
    pub join_type: JoinType,
    pub target_table: Table<'a>,
    pub main_column: ColumnRef<'a>,
    pub target_coliumn: ColumnRef<'a>,
}

#[derive(Debug, Clone, Copy)]
pub enum JoinType {
    Inner,
    Left,
    Right,
}

impl From<Table<'_>> for QueryBuilder<'_> {
    fn from(value: Table<'_>) -> Self {
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

impl QueryBuilder<'_> {
    /// Select specified columns
    pub fn select(mut self, columns: Vec<ColumnRef<'_>>) -> Self {
        self.select = columns;
        self
    }

    /// Add where condition
    pub fn where_condition(mut self, condition: Condition<'_>) -> Self {
        self.where_conditions.push(condition);
        self
    }

    /// Build the SQL query string
    pub fn build(self) -> (String, Vec<SqlParameter>) {
        let mut sql = String::new();
        let mut params = Vec::new();
        let mut param_counter = 1;
        
        // SELECT clause
        sql.push_str("SELECT ");
        let columns = self.select.iter()
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
            let (where_clause, where_params) = Self::generate_where_conditions(&self.where_conditions, &self.from_table.alias, &mut param_counter);
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

    /// Generate WHERE conditions
    fn generate_where_conditions(
        conditions: &[Condition],
        table_alias: &str,
        param_counter: &mut usize,
    ) -> (String, Vec<SqlParameter>) {
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
    pub fn to_sql(&self, param_counter: &mut usize) -> (String, Vec<SqlParameter>) {
        match self {
            Condition::Equals { column, value } => {
                let sql = format!("{} = ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![value.into()])
            }
            
            Condition::GreaterThan { column, value } => {
                let sql = format!("{} > ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![value.into()])
            }
            
            Condition::LessThan { column, value } => {
                let sql = format!("{} < ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![value.into()])
            }
            
            Condition::GreaterThanOrEqual { column, value } => {
                let sql = format!("{} >= ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![value.into()])
            }
            
            Condition::LessThanOrEqual { column, value } => {
                let sql = format!("{} <= ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![value.into()])
            }
            
            Condition::Contains { column, value } => {
                let sql = format!("{} ILIKE ${}", column.qualified(), param_counter);
                *param_counter += 1;
                (sql, vec![SqlParameter::Text(format!("%{}%", value))])
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
                let params: Vec<SqlParameter> = values
                    .iter()
                    .map(|v| v.into())
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
        }
    }
}

impl From<&ConditionValue> for SqlParameter {
    fn from(value: &ConditionValue) -> Self {
        match value {
            ConditionValue::Text(s) => SqlParameter::Text(s.clone()),
            ConditionValue::Integer(i) => SqlParameter::Integer(*i),
            ConditionValue::Boolean(b) => SqlParameter::Boolean(*b),
            ConditionValue::Null => SqlParameter::Null,
        }
    }
}

// SQL parameter that will be bound to query
#[derive(Debug, Clone)]
pub enum SqlParameter {
    Text(String),
    Integer(i64),
    Boolean(bool),
    Null,
}

impl SqlParameter {
    /// Bind to sqlx query
    pub fn bind_to_query<'q>(self, query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        match self {
            SqlParameter::Text(s) => query.bind(s),
            SqlParameter::Integer(i) => query.bind(i),
            SqlParameter::Boolean(b) => query.bind(b),
            SqlParameter::Null => query.bind::<Option<String>>(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::infrastructure::persistence::schema::Column;

    use super::*;
    
    #[test]
    fn test_simple_select() {
        let table = Table {name: "partners", alias: "t", };
        let builder = QueryBuilder::from_table(table)
            .select(vec![Cow::Owned(Column { qualifier: "t", name: "id" })])
            .where_condition(Condition::Equals { 
                 column: Cow::Owned(Column { qualifier: "t", name: "is_draft" }), 
                 value: ConditionValue::Boolean(false) });
        
        let (sql, params) = builder.build();
        
        assert!(sql.contains("SELECT t.\"id\" FROM"));
        assert!(sql.contains("WHERE t.\"is_draft\" = $1"));
    }
}