#[derive(Debug, serde::Serialize)]
pub struct DiscriminatorInfo {
    pub property_name: String,
    /// (discriminator_value, schema_name) pairs
    pub mapping: Vec<(String, String)>,
}

#[derive(Debug, serde::Serialize)]
pub struct SchemaModel {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub fields: Vec<super::resource::Field>,
    pub composition: Option<Composition>,
    pub discriminator: Option<DiscriminatorInfo>,
    pub external_docs: Option<super::resource::ExternalDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_type: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub enum Composition {
    AllOf,
    OneOf(Vec<String>),
    AnyOf(Vec<String>),
    Enum(Vec<String>),
}
