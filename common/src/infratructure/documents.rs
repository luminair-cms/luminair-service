use std::{collections::{HashMap, HashSet}, path::Path, sync::{Arc, OnceLock}};

use anyhow::{*, Context};
use serde::Deserialize;

use crate::{AttributeId, domain::{DocumentType, DocumentTypeId, DocumentTypesRegistry}, entities::{AttributeConstraints, AttributeType, DocumentField, DocumentKind, DocumentRelation, DocumentTitle, DocumentTypeInfo, DocumentTypeOptions, LocalizationId, LocalizationIdError, RelationType}};

pub fn load(schema_config_path: &str) -> Result<&'static dyn DocumentTypesRegistry, anyhow::Error> {
    let loaded = DocumentTypesRegistryAdapter::load(schema_config_path)?;
    // store loaded documents in static variable
   DOCUMENTS_REGISTRY.set(Arc::new(loaded)).expect("Failed to set documents");
    // get reference to Documents trait with static lifetime
    let documents: &'static dyn DocumentTypesRegistry = DOCUMENTS_REGISTRY.get().unwrap().as_ref();
    Ok(documents)
}

static DOCUMENTS_REGISTRY: OnceLock<Arc<dyn DocumentTypesRegistry>> = OnceLock::new();

#[derive(Debug)]
struct DocumentTypesRegistryAdapter {
    types: HashSet<&'static DocumentType>,
}

impl DocumentTypesRegistry for DocumentTypesRegistryAdapter {
    fn iterate(&self) -> Box<dyn Iterator<Item = &'static DocumentType> + '_> {
        Box::new(self.types.iter().copied())
    }

    fn get(&self, id: &DocumentTypeId) -> Option<&'static DocumentType> {
        self.types
            .get(id)
            .and_then(|idx| self.types.get(*idx).copied())
    }
}

impl DocumentTypesRegistryAdapter {
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

        let mut types = HashSet::new();
        for entry_res in entries {
            let entry =
                entry_res.map_err(|e| anyhow!("failed to read a directory entry: {}", e))?;
            let path = entry.path();
            if path.is_file() && is_json(&path) {
                let document = load_document(&path)?;
                let static_ref: &'static DocumentType = Box::leak(Box::new(document));
                types.insert(static_ref);
            }
        }

        Ok(Self { types })
    }
}

// Use DeserializeOwned so the deserialized value owns its data and does not borrow from `content`.
fn load_document(path: &Path) -> Result<DocumentType, anyhow::Error> {
    use std::fs;

    let path_str = path.to_string_lossy().into_owned();

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read entity config file '{}'", path_str))?;

    let document_record = serde_json::from_str::<DocumentRecord>(&content)
        .with_context(|| format!("failed to parse JSON entity config '{}'", path_str))?;

    let id = path
        .file_stem()
        .and_then(|os_str| os_str.to_str())
        .ok_or_else(|| anyhow!("failed to get file stem for path '{}'", path_str))?;
    (id,document_record).try_into()
}

fn is_json(path: &Path) -> bool {
    path.extension().map(|ext| ext == "json").unwrap_or(false)
}

// internal structs for Deserializing

#[derive(Clone, Debug, Deserialize)]
#[serde(bound = "'de: 'a")]
#[serde(rename_all = "camelCase")]
struct DocumentRecord<'a> {
    #[serde(alias = "type")]
    kind: DocumentKind,
    info: DocumentInfoRecord<'a>,
    options: Option<DocumentOptionsRecord<'a>>,
    attributes: HashMap<&'a str, AttributeRecord<'a>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(bound = "'de: 'a")]
#[serde(rename_all = "camelCase")]
struct DocumentInfoRecord<'a> {
    title: &'a str,
    description: Option<&'a str>,
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
#[serde(bound = "'de: 'a")]
#[serde(rename_all = "camelCase", untagged)]
enum AttributeRecord<'a> {
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
        #[serde(alias = "relation")]
        relation_type: RelationType,
        target: &'a str,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(bound = "'de: 'a")]
#[serde(rename_all = "camelCase")]
struct AttributeConstraintsRecord<'a> {
    pattern: Option<&'a str>,
    minimal_length: Option<usize>,
    maximal_length: Option<usize>,
}

// conversion into document model

impl<'a> TryFrom<(&'a str, DocumentRecord<'a>)> for DocumentType {
    type Error = anyhow::Error;

    fn try_from(value: (&'a str, DocumentRecord<'a>)) -> Result<Self, Self::Error> {
        let id = DocumentTypeId::try_new(value.0)?;
        let record = &value.1;
        let kind = record.kind;
        let info = DocumentTypeInfo::try_from(&record.info)?;
        let options = record.options.as_ref().map(DocumentTypeOptions::try_from).transpose()?;

        let mut fields = HashMap::new();
        let mut relations = HashMap::new();

        for attribute in record.attributes.iter() {
            let id = AttributeId::try_new(*attribute.0)?;
            let record = attribute.1;

            match record {
                AttributeRecord::Field {
                    attribute_type,
                    unique,
                    required,
                    localized,
                    constraints,
                } => {
                    let constraints = constraints.as_ref().map(AttributeConstraints::from);
                    let field = DocumentField {
                        attribute_type: *attribute_type,
                        unique: *unique,
                        required: *required,
                        localized: *localized,
                        constraints,
                    };
                    fields.insert(id, field);
                }
                AttributeRecord::Relation {
                    relation_type,
                    target
                } => {
                    let target = DocumentTypeId::try_new(target.to_owned())?;
                    
                    let relation = DocumentRelation {
                        relation_type: *relation_type,
                        target
                    };
                    relations.insert(id, relation);
                }
            }
        }

        Ok(Self {
            id,
            kind,
            info,
            options,
            fields,
            relations,
        })
    }
}

impl<'a> TryFrom<&DocumentOptionsRecord<'a>> for DocumentTypeOptions {
    type Error = anyhow::Error;

    fn try_from(value: &DocumentOptionsRecord<'a>) -> Result<Self, Self::Error> {
        let draft_and_publish = value.draft_and_publish;
        let localizations: Result<Vec<LocalizationId>, LocalizationIdError> = value
            .localizations
            .iter()
            .map(|localization| LocalizationId::try_new(localization.to_owned()))
            .collect();
        Ok(Self {
            draft_and_publish,
            localizations: localizations?,
        })
    }
}

impl<'a> TryFrom<&DocumentInfoRecord<'a>> for DocumentTypeInfo {
    type Error = anyhow::Error;

    fn try_from(value: &DocumentInfoRecord<'a>) -> Result<Self, Self::Error> {
        let title = DocumentTitle::try_new(value.title)?;
        let description = value.description.map(String::from);
        let singular_name = DocumentTypeId::try_new(value.singular_name)?;
        let plural_name = DocumentTypeId::try_new(value.plural_name)?;

        Ok(Self {
            title,
            description,
            singular_name,
            plural_name,
        })
    }
}

impl<'a> From<&AttributeConstraintsRecord<'a>> for AttributeConstraints {
    fn from(value: &AttributeConstraintsRecord<'a>) -> Self {
        Self {
            pattern: value.pattern.map(String::from),
            minimal_length: value.minimal_length,
            maximal_length: value.maximal_length,
        }
    }
}
