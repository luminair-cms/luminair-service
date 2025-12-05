use std::{collections::HashSet, path::Path, sync::Arc};

use anyhow::{Context, anyhow};
use serde::Deserialize;

use crate::domain::{
    document_attributes::{
        Attribute, AttributeBody, AttributeConstraints, AttributeId, AttributeType, RelationType,
    },
    documents::{
        Document, DocumentDescription, DocumentId, DocumentInfo, DocumentOptions, DocumentTitle,
        DocumentType, Documents, LocalizationId, LocalizationIdError,
    },
};

#[derive(Clone)]
pub struct DocumentsAdapter {
    internal: Arc<Internal>,
}

struct Internal {
    documents: HashSet<Document>,
}

impl Documents for DocumentsAdapter {
    fn documents(&self) -> impl Iterator<Item = &Document> {
        self.internal.documents.iter()
    }

    fn get_document(&self, id: &DocumentId) -> Option<&Document> {
        self.internal.documents.get(id)
    }
}

impl DocumentsAdapter {
    pub fn load(schema_config_path: &str) -> Result<Self, anyhow::Error> {
        use std::fs;
        use std::path::Path;

        let dir_path = Path::new(schema_config_path);

        tracing::debug!("Loading from {}", dir_path.to_string_lossy());

        let entries = fs::read_dir(dir_path).with_context(|| {
            format!(
                "failed to read schema config directory: {}",
                dir_path.to_string_lossy()
            )
        })?;

        let mut documents: HashSet<Document> = HashSet::new();
        for entry_res in entries {
            let entry =
                entry_res.map_err(|e| anyhow!("failed to read a directory entry: {}", e))?;
            let path = entry.path();
            if path.is_file() && is_json(&path) {
                let document = load_document(&path)?;
                documents.insert(document);
            }
        }

        let internal = Arc::new(Internal { documents });
        Ok(Self { internal })
    }
}

// Use DeserializeOwned so the deserialized value owns its data and does not borrow from `content`.
fn load_document(path: &Path) -> Result<Document, anyhow::Error> {
    use std::fs;

    let path_str = path.to_string_lossy().into_owned();

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read entity config file '{}'", path_str))?;

    let document_record = serde_json::from_str::<DocumentRecord>(&content)
        .with_context(|| format!("failed to parse JSON entity config '{}'", path_str))?;

    document_record.try_into()
}

fn is_json(path: &Path) -> bool {
    path.extension().map(|ext| ext == "json").unwrap_or(false)
}

// internal structs for Deserializing

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentRecord<'a> {
    id: &'a str,
    #[serde(alias = "type")]
    document_type: DocumentType,
    info: DocumentInfoRecord<'a>,
    options: Option<DocumentOptionsRecord<'a>>,
    attributes: Vec<AttributeRecord<'a>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentInfoRecord<'a> {
    title: &'a str,
    description: &'a str,
    singular_name: &'a str,
    plural_name: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(bound = "'de: 'a")]
#[serde(rename_all = "camelCase")]
struct DocumentOptionsRecord<'a> {
    #[serde(default)]
    draft_and_publish: bool,
    #[serde(default)]
    localizations: Vec<&'a str>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttributeRecord<'a> {
    id: &'a str,
    #[serde(flatten)]
    body: AttributeBodyRecord<'a>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
enum AttributeBodyRecord<'a> {
    Field {
        #[serde(alias = "type")]
        attribute_type: AttributeType,
        #[serde(default)]
        unique: bool,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        localized: bool,
        constraints: Option<AttributeConstraintsRecord<'a>>,
    },
    Relation {
        relation: RelationType,
        target: &'a str,
        #[serde(default)]
        ordering: bool,
        mapped_by: Option<&'a str>,
        inversed_by: Option<&'a str>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttributeConstraintsRecord<'a> {
    pattern: Option<&'a str>,
    minimal_length: Option<usize>,
    maximal_length: Option<usize>,
}

// conversion into document model

impl<'a> TryFrom<DocumentRecord<'a>> for Document {
    type Error = anyhow::Error;

    fn try_from(value: DocumentRecord<'a>) -> Result<Self, Self::Error> {
        let id = DocumentId::try_new(value.id)?;
        let document_type = value.document_type;
        let info = DocumentInfo::try_from(value.info)?;
        let options = value.options.map(DocumentOptions::try_from).transpose()?;
        let attributes: Result<Vec<Attribute>, anyhow::Error> = value
            .attributes
            .into_iter()
            .map(Attribute::try_from)
            .collect();
        Ok(Self {
            id,
            document_type,
            info,
            options,
            attributes: attributes?,
        })
    }
}

impl<'a> TryFrom<DocumentOptionsRecord<'a>> for DocumentOptions {
    type Error = anyhow::Error;

    fn try_from(value: DocumentOptionsRecord<'a>) -> Result<Self, Self::Error> {
        let draft_and_publish = value.draft_and_publish;
        let localizations: Result<Vec<LocalizationId>, LocalizationIdError> = value
            .localizations
            .into_iter()
            .map(LocalizationId::try_new)
            .collect();
        Ok(Self {
            draft_and_publish,
            localizations: localizations?,
        })
    }
}

impl<'a> TryFrom<DocumentInfoRecord<'a>> for DocumentInfo {
    type Error = anyhow::Error;

    fn try_from(value: DocumentInfoRecord<'a>) -> Result<Self, Self::Error> {
        let title = DocumentTitle::try_new(value.title)?;
        let description = DocumentDescription::try_new(value.description)?;
        let singular_name = DocumentId::try_new(value.singular_name)?;
        let plural_name = DocumentId::try_new(value.plural_name)?;

        Ok(Self {
            title,
            description,
            singular_name,
            plural_name,
        })
    }
}

impl<'a> TryFrom<AttributeRecord<'a>> for Attribute {
    type Error = anyhow::Error;

    fn try_from(value: AttributeRecord<'a>) -> Result<Self, Self::Error> {
        let id = AttributeId::try_new(value.id)?;
        let body = AttributeBody::try_from(value.body)?;
        Ok(Self { id, body })
    }
}

impl<'a> TryFrom<AttributeBodyRecord<'a>> for AttributeBody {
    type Error = anyhow::Error;

    fn try_from(value: AttributeBodyRecord<'a>) -> Result<Self, Self::Error> {
        let result = match value {
            AttributeBodyRecord::Field {
                attribute_type,
                unique,
                required,
                localized,
                constraints,
            } => {
                let constraints = constraints.map(AttributeConstraints::from);
                Self::Field {
                    attribute_type,
                    unique,
                    required,
                    localized,
                    constraints,
                }
            }
            AttributeBodyRecord::Relation {
                relation,
                target,
                ordering,
                mapped_by,
                inversed_by,
            } => {
                let target = DocumentId::try_new(target)?;
                let mapped_by = mapped_by.map(AttributeId::try_new).transpose()?;
                let inversed_by = inversed_by.map(AttributeId::try_new).transpose()?;
                Self::Relation {
                    relation_type: relation,
                    target,
                    ordering,
                    mapped_by,
                    inversed_by,
                }
            }
        };
        Ok(result)
    }
}

impl<'a> From<AttributeConstraintsRecord<'a>> for AttributeConstraints {
    fn from(value: AttributeConstraintsRecord<'a>) -> Self {
        Self {
            pattern: value.pattern.map(String::from),
            minimal_length: value.minimal_length,
            maximal_length: value.maximal_length,
        }
    }
}
