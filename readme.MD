## Associations

- One way: collection A has one row of collection B
- One to one: collection A has one row of collection B, and collection B has one row of collection A
- One to many: collection A has many rows of collection B
- Many to one: collection B has many rows of collection A
- Many to many: collection A has many rows of collection B, and collection B has many rows of collection A
- Many way: collection A has many rows of collection B

## mappedBy and inversedBy

- mappedBy and inversedBy are used to define the relationship between the owning and inverse sides of a bidirectional association. 
- mappedBy: This attribute is used on the inverse side of the relationship. It specifies the property name on the owning entity that holds the association. In a Many-to-Many relationship, the inverse side does not manage the foreign keys in the join table directly. Instead, it relies on the owning side to define and manage the relationship.
- inversedBy: This attribute is used on the owning side of the relationship. It specifies the property name on the inverse entity that holds the association. The owning side is responsible for managing the join table and the foreign keys that establish the Many-to-Many relationship.

## The following general rules apply:

- Relationships may be bidirectional or unidirectional.
- A bidirectional relationship has both an owning side and an inverse side
- A unidirectional relationship only has an owning side.
- Doctrine will only check the owning side of an association for changes.

# Bidirectional Associations

## The following rules apply to bidirectional associations:

- The inverse side has to have the mappedBy attribute of the OneToOne, OneToMany, or ManyToMany mapping declaration. The mappedBy attribute contains the name of the association-field on the owning side.
- The owning side has to have the inversedBy attribute of the OneToOne, ManyToOne, or ManyToMany mapping declaration. The inversedBy attribute contains the name of the association-field on the inverse-side.
- ManyToOne is always the owning side of a bidirectional association.
- OneToMany is always the inverse side of a bidirectional association.
- The owning side of a OneToOne association is the entity with the table containing the foreign key.
- You can pick the owning side of a many-to-many association yourself.