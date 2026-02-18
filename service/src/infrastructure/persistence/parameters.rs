use std::fmt::Debug;

use sqlx::{Arguments, postgres::PgArguments, types::Uuid};

#[derive(Clone,Copy,Debug)]
pub struct QueryParameterRef { 
    index: usize
}

pub struct QueryParametersHolder {
    // Use Option so we can `take()` by index without shifting the vector
    parameters: Vec<Option<Box<dyn QueryParameter>>>,
}

pub trait QueryParameter: Debug + Send {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments);
}

impl QueryParametersHolder {
    pub fn new() -> Self {
        Self { parameters: Vec::new() }
    }
    
    pub fn bind<T>(&mut self, value: T) -> QueryParameterRef 
    where T: Into<Box<dyn QueryParameter>>
    {
        self.parameters.push(Some(value.into()));
        let index = self.parameters.len() - 1;
        QueryParameterRef { index }
    }
    
    pub fn generate_args(mut self, ordered_refs: &[QueryParameterRef]) -> PgArguments {
        let mut args = PgArguments::default();
        
        for param_ref in ordered_refs {
            let slot = self
                .parameters
                .get_mut(param_ref.index)
                .unwrap_or_else(|| panic!("invalid parameter index {}", param_ref.index));
            let boxed = slot.take().expect("parameter was already consumed");
            boxed.add_to_args(&mut args); // ownership moved here, move inner values with no clone
        }
                
        args
    }
}

// Text (String)

#[derive(Debug, Clone)]
struct TextQueryParameter (String);

impl QueryParameter for TextQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
            // we own the String now; move it into args without cloning
            let TextQueryParameter(value) = *self;
            args.add(value);
        }
}

impl From<String> for TextQueryParameter {
    fn from(value: String) -> Self {
        Self (value)
    }
}

impl Into<Box<dyn QueryParameter>> for String {
    fn into(self) -> Box<dyn QueryParameter> {
        Box::new(TextQueryParameter::from(self))
    }
}

impl From<&str> for TextQueryParameter {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Into<Box<dyn QueryParameter>> for &str {
    fn into(self) -> Box<dyn QueryParameter> {
        Box::new(TextQueryParameter::from(self))
    }
}

// Integer (i64)

#[derive(Debug, Clone)]
struct IntegerQueryParameter (i64);

impl From<i64> for IntegerQueryParameter {
    fn from(value: i64) -> Self {
        Self (value)
    }
}

impl Into<Box<dyn QueryParameter>> for i64 {
    fn into(self) -> Box<dyn QueryParameter> {
        Box::new(IntegerQueryParameter::from(self))
    }
}

impl QueryParameter for IntegerQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let IntegerQueryParameter(value) = *self;
        args.add(value);
    }
}

// Boolean

#[derive(Debug, Clone)]
struct BooleanQueryParameter (bool);

impl From<bool> for BooleanQueryParameter {
    fn from(value: bool) -> Self {
        Self (value)
    }
}

impl Into<Box<dyn QueryParameter>> for bool {
    fn into(self) -> Box<dyn QueryParameter> {
        Box::new(BooleanQueryParameter::from(self))
    }
}

impl QueryParameter for BooleanQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let BooleanQueryParameter(value) = *self;
        args.add(value);
    }
}

// UUid

#[derive(Debug, Clone)]
struct UuidQueryParameter (Uuid);

impl From<Uuid> for UuidQueryParameter {
    fn from(value: Uuid) -> Self {
        Self (value)
    }
}

impl Into<Box<dyn QueryParameter>> for Uuid {
    fn into(self) -> Box<dyn QueryParameter> {
        Box::new(UuidQueryParameter::from(self))
    }
}

impl QueryParameter for UuidQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let UuidQueryParameter(value) = *self;
        args.add(value);
    }
}

// NULL
// TODO: because bind has type, all query params can be NULL
 
#[derive(Debug, Clone)]
pub struct NullQueryParameter ();

impl QueryParameter for NullQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let null: Option<String> = None;
        args.add(null);
    }
}

// Vector of i64

#[derive(Debug, Clone)]
pub struct IntegerVectorQueryParameter (Vec<i64>);

impl From<Vec<i64>> for IntegerVectorQueryParameter {
    fn from(value: Vec<i64>) -> Self {
        Self (value)
    }
}

impl Into<Box<dyn QueryParameter>> for Vec<i64> {
    fn into(self) -> Box<dyn QueryParameter> {
        Box::new(IntegerVectorQueryParameter::from(self))
    }
}

impl QueryParameter for IntegerVectorQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let IntegerVectorQueryParameter(value) = *self;
        args.add(value);
    }
}