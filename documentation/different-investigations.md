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

# Associations

- One way: collection A has one row of collection B
- One to one: collection A has one row of collection B, and collection B has one row of collection A
- One to many: collection A has many rows of collection B
- Many to one: collection B has many rows of collection A
- Many to many: collection A has many rows of collection B, and collection B has many rows of collection A
- Many way: collection A has many rows of collection B

### mappedBy and inversedBy

- mappedBy and inversedBy are used to define the relationship between the owning and inverse sides of a bidirectional association.
- mappedBy: This attribute is used on the inverse side of the relationship. It specifies the property name on the owning entity that holds the association. In a Many-to-Many relationship, the inverse side does not manage the foreign keys in the join table directly. Instead, it relies on the owning side to define and manage the relationship.
- inversedBy: This attribute is used on the owning side of the relationship. It specifies the property name on the inverse entity that holds the association. The owning side is responsible for managing the join table and the foreign keys that establish the Many-to-Many relationship.

### The following general rules apply:

- Relationships may be bidirectional or unidirectional.
- A bidirectional relationship has both an owning side and an inverse side
- A unidirectional relationship only has an owning side.
- Doctrine will only check the owning side of an association for changes.

## Bidirectional Associations

### The following rules apply to bidirectional associations:

- The inverse side has to have the mappedBy attribute of the OneToOne, OneToMany, or ManyToMany mapping declaration. The mappedBy attribute contains the name of the association-field on the owning side.
- The owning side has to have the inversedBy attribute of the OneToOne, ManyToOne, or ManyToMany mapping declaration. The inversedBy attribute contains the name of the association-field on the inverse-side.
- ManyToOne is always the owning side of a bidirectional association.
- OneToMany is always the inverse side of a bidirectional association.
- The owning side of a OneToOne association is the entity with the table containing the foreign key.
- You can pick the owning side of a many-to-many association yourself.
