### Using ANY for a single parameter

The most straightforward way to use a Rust vector in a query is with the ANY operator in SQL, which checks if a value is present in an array. sqlx automatically handles the conversion of a Rust slice or vector to a PostgreSQL array parameter.

```rust
  use sqlx::{postgres::PgPool, Error};
  
  async fn select_users_by_ids(pool: &PgPool, user_ids: Vec<i32>) -> Result<(), Error> {
      // Pass the vector as a slice reference &[]
      let rows = sqlx::query!("
          SELECT id, username
          FROM users
          WHERE id = ANY($1)
      ",
      &user_ids // Bind the vector as a single parameter
      )
      .fetch_all(pool)
      .await?;
  
      for row in rows {
          println!("User: {}, {}", row.id, row.username.unwrap_or_default());
      }
  
      Ok(())
  }
  ```
  
### Using IN clause with dynamic parameter expansion (for non-array types)

If you need to use the IN clause with a variable number of parameters (which doesn't rely on the PostgreSQL array type but rather dynamic query building), you must manually build the query string and bind parameters iteratively to prevent SQL injection.

```rust
  use sqlx::{postgres::PgPool, query, Error};
  
  async fn select_users_in_list(pool: &PgPool, user_ids: Vec<i32>) -> Result<(), Error> {
      if user_ids.is_empty() {
          // Handle empty vector case if needed
          return Ok(());
      }
  
      // Generate the $1, $2, $3... placeholders
      let placeholders: String = (1..=user_ids.len())
          .map(|i| format!("${}", i))
          .collect::<Vec<String>>()
          .join(", ");
  
      let query_str = format!("
          SELECT id, username
          FROM users
          WHERE id IN ({})
      ", placeholders);
  
      // Build the query and bind parameters one by one
      let mut query = query(&query_str);
      for id in &user_ids {
          query = query.bind(id);
      }
  
      let rows: Vec<(i32, String)> = query
          .map(|row: sqlx::postgres::PgRow| {
              // Manually extract values or use a struct with #[derive(FromRow)]
              (row.get("id"), row.get("username"))
          })
          .fetch_all(pool)
          .await?;
  
      for user in rows {
          println!("User: {}, {}", user.0, user.1);
      }
  
      Ok(())
  }
```

### Key Points:

For simple equality checks against a list in Postgres, the ANY($1) approach is cleaner and safer as it treats the entire vector as a single bound parameter, leveraging Postgres array types.
For dynamically generating IN clauses with individual parameters, you need to construct the query string at runtime and use a loop to bind each element.