#[derive(Debug, serde::Serialize)]
pub struct DiscriminatorInfo {
    pub property_name: String,
    /// (discriminator_value, schema_name) pairs
    pub mapping: Vec<(String, String)>,
}

#[derive(Debug, serde::Serialize)]
pub struct SchemaModel {
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<super::resource::Field>,
    pub composition: Option<Composition>,
    pub discriminator: Option<DiscriminatorInfo>,
    pub external_docs: Option<super::resource::ExternalDoc>,
}

#[derive(Debug, serde::Serialize)]
pub enum Composition {
    AllOf,
    OneOf(Vec<String>),
    AnyOf(Vec<String>),
    Enum(Vec<String>),
}
