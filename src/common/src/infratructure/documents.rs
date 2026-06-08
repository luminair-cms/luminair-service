use std::{collections::{HashMap, HashSet}, path::Path, sync::{Arc, OnceLock}};

use anyhow::{*, Context};
use serde::Deserialize;

use crate::{AttributeId, domain::{DocumentType, DocumentTypeId, DocumentTypesRegistry}, entities::{FieldType, DocumentField, DocumentKind, DocumentRelation, DocumentTitle, DocumentTypeInfo, DocumentTypeOptions, LocalizationId, LocalizationIdError, RelationType}, DocumentTypeApiId};
use crate::entities::FieldConstraint;

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
    map: HashMap<String, &'static DocumentType>,
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

    fn lookup(&self, api_id: &DocumentTypeApiId) -> Option<&'static DocumentType> {
        self.map.get(api_id.as_ref()).copied()
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

        let mut map = HashMap::new();
        for dt in types.iter() {
            let api_id = match dt.kind {
                DocumentKind::SingleType => 
                    dt.info.singular_name.as_ref().to_string(),
                DocumentKind::Collection => 
                    dt.info.plural_name.as_ref().to_string()
            };
            map.insert(api_id, *dt);
        }

        Ok(Self { types, map })
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

#[cfg(test)]
mod tests {
        use super::*;
        use std::fs::{self, File};
        use std::io::Write;
        use std::path::PathBuf;

        #[test]
        fn is_json_checks_extension() {
                assert!(is_json(&Path::new("/tmp/a.json")));
                assert!(!is_json(&Path::new("/tmp/a.txt")));
                assert!(!is_json(&Path::new("/tmp/a")));
        }

        // The more comprehensive parsing test was moved to an integration test using
        // the `tempfile` crate to ensure safe cleanup.
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
        field_type: FieldType,
        #[serde(default)]
        unique: bool,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        constraints: HashSet<FieldConstraint>,
    },
    Relation {
        #[serde(alias = "relation")]
        relation_type: RelationType,
        target: &'a str,
    },
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

        let mut fields = HashSet::new();
        let mut relations = HashSet::new();

        for attribute in record.attributes.iter() {
            let id = AttributeId::try_new(*attribute.0)?;
            let record = attribute.1;

            match record {
                AttributeRecord::Field {
                    field_type,
                    unique,
                    required,
                    constraints,
                } => {
                    let field_type = *field_type;

                    let constraints_are_valid = constraints.iter().all(|constraint| 
                        constraint.is_applicable_for(field_type)
                    );
                    if !constraints_are_valid {
                        return Err(anyhow!("Invalid constraints for field '{}': constraints are not applicable for field type '{:?}'", id, field_type));
                    }
                    let constraints = constraints.into_iter().map(|it|it.clone()).collect();
                    
                    let field = DocumentField {
                        id,
                        field_type,
                        unique: *unique,
                        required: *required,
                        constraints,
                    };
                    fields.insert(field);
                }
                AttributeRecord::Relation {
                    relation_type,
                    target
                } => {
                    let target = DocumentTypeId::try_new(target.to_owned())?;
                    
                    let relation = DocumentRelation {
                        id,
                        relation_type: *relation_type,
                        target
                    };
                    relations.insert(relation);
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
