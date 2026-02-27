use crate::domain::{document::content::ContentValue, sql::SqlParameterRef};
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgArguments, types::Uuid, Arguments};
use serde_json;
use std::fmt::Debug;

pub struct SqlParametersHolder {
    // Use Option so we can `take()` by index without shifting the vector
    parameters: Vec<Option<Box<dyn SqlParameter>>>,
}

pub trait SqlParameter: Debug + Send {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments);
}

impl SqlParametersHolder {
    pub fn new() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }

    pub fn bind<T>(&mut self, value: T) -> SqlParameterRef
    where
        T: Into<Box<dyn SqlParameter>>,
    {
        self.parameters.push(Some(value.into()));
        let index = self.parameters.len() - 1;
        SqlParameterRef::from(index)
    }
    
    pub fn bind_null(&mut self) -> SqlParameterRef {
        self.parameters.push(Some(NullQueryParameter::new()));
        let index = self.parameters.len() - 1;
        SqlParameterRef::from(index)
    }

    pub fn generate_args(mut self, ordered_refs: &[SqlParameterRef]) -> PgArguments {
        let mut args = PgArguments::default();

        for param_ref in ordered_refs {
            let slot = self
                .parameters
                .get_mut(param_ref.index())
                .unwrap_or_else(|| panic!("invalid parameter index {}", param_ref.index()));
            let boxed = slot.take().expect("parameter was already consumed");
            boxed.add_to_args(&mut args); // ownership moved here, move inner values with no clone
        }

        args
    }
}

// TODO: bind domain values directly

// Text (String)

#[derive(Debug, Clone)]
struct TextQueryParameter(String);

impl SqlParameter for TextQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        // we own the String now; move it into args without cloning
        let TextQueryParameter(value) = *self;
        // TODO: handle Result from add() in case of error
        args.add(value);
    }
}

impl From<String> for TextQueryParameter {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for String {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(TextQueryParameter::from(self))
    }
}

impl From<&str> for TextQueryParameter {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Into<Box<dyn SqlParameter>> for &str {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(TextQueryParameter::from(self))
    }
}

// Integer (i64)

#[derive(Debug, Clone)]
struct IntegerQueryParameter(i64);

impl From<i64> for IntegerQueryParameter {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for i64 {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(IntegerQueryParameter::from(self))
    }
}

impl SqlParameter for IntegerQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let IntegerQueryParameter(value) = *self;
        args.add(value);
    }
}

// Decimal (f64)

#[derive(Debug, Clone)]
struct DecimalQueryParameter(f64);

impl From<f64> for DecimalQueryParameter {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for f64 {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(DecimalQueryParameter::from(self))
    }
}

impl SqlParameter for DecimalQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let DecimalQueryParameter(value) = *self;
        args.add(value);
    }
}

// Date (chrono::NaiveDate)

#[derive(Debug, Clone)]
struct DateQueryParameter(chrono::NaiveDate);

impl From<chrono::NaiveDate> for DateQueryParameter {
    fn from(value: chrono::NaiveDate) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for chrono::NaiveDate {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(DateQueryParameter::from(self))
    }
}

impl SqlParameter for DateQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let DateQueryParameter(value) = *self;
        args.add(value);
    }
}

// Boolean

#[derive(Debug, Clone)]
struct BooleanQueryParameter(bool);

impl From<bool> for BooleanQueryParameter {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for bool {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(BooleanQueryParameter::from(self))
    }
}

impl SqlParameter for BooleanQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let BooleanQueryParameter(value) = *self;
        args.add(value);
    }
}

// UUid

#[derive(Debug, Clone)]
struct UuidQueryParameter(Uuid);

impl From<Uuid> for UuidQueryParameter {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for Uuid {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(UuidQueryParameter::from(self))
    }
}

impl SqlParameter for UuidQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let UuidQueryParameter(value) = *self;
        args.add(value);
    }
}

// DateTime

#[derive(Debug, Clone)]
struct DateTimeQueryParameter(DateTime<Utc>);

impl From<DateTime<Utc>> for DateTimeQueryParameter {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for DateTime<Utc> {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(DateTimeQueryParameter::from(self))
    }
}

impl SqlParameter for DateTimeQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let DateTimeQueryParameter(value) = *self;
        args.add(value);
    }
}

// NULL

#[derive(Debug, Clone)]
pub struct NullQueryParameter();

impl NullQueryParameter {
    pub fn new() -> Box<dyn SqlParameter> {
        Box::new(Self())
    }
}

impl SqlParameter for NullQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let null: Option<String> = None;
        args.add(null);
    }
}

// Vector of i64

#[derive(Debug, Clone)]
pub struct IntegerVectorQueryParameter(Vec<i64>);

impl From<Vec<i64>> for IntegerVectorQueryParameter {
    fn from(value: Vec<i64>) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for Vec<i64> {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(IntegerVectorQueryParameter::from(self))
    }
}

impl SqlParameter for IntegerVectorQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        let IntegerVectorQueryParameter(value) = *self;
        args.add(value);
    }
}

#[derive(Debug, Clone)]
struct ContentValueQueryParameter(ContentValue);

impl From<ContentValue> for ContentValueQueryParameter {
    fn from(value: ContentValue) -> Self {
        Self(value)
    }
}

impl Into<Box<dyn SqlParameter>> for ContentValue {
    fn into(self) -> Box<dyn SqlParameter> {
        Box::new(ContentValueQueryParameter::from(self))
    }
}

impl SqlParameter for ContentValueQueryParameter {
    fn add_to_args(self: Box<Self>, args: &mut PgArguments) {
        use crate::domain::document::content::{ContentValue, DomainValue};
        let ContentValueQueryParameter(value) = *self;

        match value {
            ContentValue::Null => {
                let null: Option<String> = None;
                let _ = args.add(null);
            }
            ContentValue::Scalar(dv) => {
                match dv {
                    DomainValue::Text(s) => { let _ = args.add(s); }
                    DomainValue::Integer(i) => { let _ = args.add(i); }
                    DomainValue::Decimal(f) => { let _ = args.add(f); }
                    DomainValue::Boolean(b) => { let _ = args.add(b); }
                    DomainValue::Date(d) => { let _ = args.add(d); }
                    DomainValue::DateTime(dt) => { let _ = args.add(dt); }
                    DomainValue::Uuid(u) => { let _ = args.add(u); }
                    DomainValue::Email(e) => { let _ = args.add(e.as_ref()); }
                    DomainValue::Url(u) => { let _ = args.add(u.as_ref()); }
                    DomainValue::Json(map) => {
                        let json = serde_json::to_value(map).unwrap();
                        let _ = args.add(json.to_string());
                    }
                }
            }
            ContentValue::LocalizedText(map) => {
                let json = serde_json::to_value(map).unwrap();
                let _ = args.add(json.to_string());
            }
        }
    }
}
