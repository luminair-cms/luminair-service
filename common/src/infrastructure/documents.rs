use std::{path::Path};
use std::collections::HashSet;
use std::sync::RwLock;
use anyhow::{Context, anyhow};
use serde::Deserialize;

use crate::domain::{attributes::{
    Attribute, AttributeConstraints, AttributeType,
}, documents::{
    Document, DocumentDescription, DocumentInfo, DocumentOptions, DocumentTitle,
    DocumentType, Documents, LocalizationId, LocalizationIdError,
}, AttributeId, DocumentId};
use crate::domain::relations::{Relation, RelationId, RelationTarget, RelationType};

#[derive(Debug)]
pub(crate) struct DocumentsAdapter {
    // Store leaked &'static Document so we can keep stable references in relations
    documents: HashSet<&'static Document>,
}

impl Documents for DocumentsAdapter {
    fn documents(&self) -> Box<dyn Iterator<Item = &Document> + '_> {
        // self.documents holds &'static Document, expose as &Document iterator
        Box::new(self.documents.iter().copied())
    }
    fn get_document(&self, id: &DocumentId) -> Option<&Document> {
        self.documents.get(id).copied()
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

        let mut documents: HashSet<&'static Document> = HashSet::new();
        for entry_res in entries {
            let entry =
                entry_res.map_err(|e| anyhow!("failed to read a directory entry: {}", e))?;
            let path = entry.path();
            if path.is_file() && is_json(&path) {
                let document = load_document(&path)?;
                // Leak the document to get an &'static reference that can be stored in relations
                let leaked: &'static Document = Box::leak(Box::new(document));
                documents.insert(leaked);
            }
        }

        Ok(Self { documents })
    }
    
    pub fn initiate(&self) -> Result<(), anyhow::Error> {
        for document in self.documents.iter().copied() {
            for relation in &document.relations {
                {
                    let mut target = relation.target.write().unwrap();
                    if let RelationTarget::Id(target_id) = &*target {
                        let found = self
                            .documents
                            .get(target_id)
                            .context(format!("Target document not found: {} from {}.{}", target_id, document.id, relation.id))?;

                        *target = RelationTarget::Ref(found);
                    }
                }
            }
        }
        Ok(())
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
    #[serde(default)]
    relations: Vec<RelationRecord<'a>>
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
    #[serde(alias = "type")]
    attribute_type: AttributeType,
    #[serde(default)]
    unique: bool,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    localized: bool,
    constraints: Option<AttributeConstraintsRecord<'a>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelationRecord<'a> {
    id: &'a str,
    relation: RelationType,
    target: &'a str,
    #[serde(default)]
    ordering: bool
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
        let relations: Result<Vec<Relation>, anyhow::Error> = value
            .relations
            .into_iter()
            .map(Relation::try_from)
            .collect();

        // TODO: validate uniqueness of attributes/relations id

        Ok(Self {
            id,
            document_type,
            info,
            options,
            attributes: attributes?,
            relations: relations?,
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
        let constraints = value.constraints.map(AttributeConstraints::from);

        Ok(Self {
            id,
            attribute_type: value.attribute_type,
            unique: value.unique,
            required: value.required,
            localized: value.localized,
            constraints
        })
    }
}

impl<'a> TryFrom<RelationRecord<'a>> for Relation {
    type Error = anyhow::Error;

    fn try_from(value: RelationRecord<'a>) -> Result<Self, Self::Error> {
        let id = RelationId::try_new(value.id)?;
        let target = DocumentId::try_new(value.target)?;
        Ok(Self {
            id,
            relation_type: value.relation,
            target: RwLock::new(RelationTarget::Id(target)),
            // target_singular_name: RefCell::new(None),
            ordering: value.ordering
        })
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
