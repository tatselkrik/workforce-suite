//! Fresh protected staff directory for operational assignments, never Account Manager data.
use crate::{access::Access,recruiting::Interviewer};
use anyhow::{Result,ensure};
use serde::Deserialize;
use uuid::Uuid;
#[derive(Clone,Debug,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Directory {
 pub schema_version:u16,pub company_id:Uuid,pub user_id:Uuid,pub runtime_ready:bool,
 pub person_count:usize,pub people:Vec<Interviewer>,
}
impl Directory {
 pub fn validate(&self,access:&Access)->Result<()> {
  ensure!(self.schema_version==super::SCHEMA_VERSION && self.company_id==access.company_id && self.user_id==access.user_id && self.runtime_ready && access.uses_desktop() && access.staff_context.as_ref().is_some_and(|c|c.runtime_ready),"Staff assignment directory is outside the current protected scope.");
  ensure!(self.person_count==self.people.len() && self.people.len()<=1000,"Staff assignment response is incomplete or exceeds its budget.");
  let mut ids=std::collections::BTreeSet::new();
  for person in &self.people {
   ensure!(!person.employee_id.is_nil() && ids.insert(person.employee_id) && !person.full_name.trim().is_empty() && person.full_name.chars().count()<=240 && !person.full_name.chars().any(char::is_control) && person.roles.as_ref().is_some_and(|roles|roles.compatible_base_role()==person.role && roles.uses_desktop()),"Invalid or overbroad staff assignment entry.");
  }
  Ok(())
 }
}
