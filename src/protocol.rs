use serde::Serialize;

#[derive(Serialize)]
pub struct ModuleUpdate<'a> {
    pub r#type: &'a str,
    pub module_id: &'a str,
    pub api_version: u32,
}
