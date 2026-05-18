use sea_query::{Expr, Value};
use serde_json::json;
use crate::domain::document::content::{ContentValue, DomainValue};

impl From<&ContentValue> for Expr {
    fn from(value: &ContentValue) -> Self {
        match value {
            ContentValue::Scalar(dv) => match dv {
                DomainValue::Text(s) => s.into(),
                DomainValue::Integer(i) => (*i).into(),
                DomainValue::Decimal(d) => (*d).into(),
                DomainValue::Boolean(b) => (*b).into(),
                DomainValue::Date(d) => (*d).into(),
                DomainValue::DateTime(dt) => (*dt).into(),
                DomainValue::Uuid(v) => (*v).into(),
                DomainValue::Json(j) => json!(j).into(),
                DomainValue::Email(email) => email.as_ref().into(),
                DomainValue::Url(url) => url.as_ref().into(),
            },
            ContentValue::LocalizedText(map) => json!(map).into(),
            ContentValue::Null => Expr::null(),
        }
    }
}