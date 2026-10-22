//! Applicant foundation for the next hiring module. No employee/Auth conversion.
use crate::{access::Access,domain::{Snapshot,Mode,parse_date}};
use anyhow::{Result,ensure,Context};
use chrono::NaiveDate;
use serde::{Deserialize,Serialize};
use uuid::Uuid;
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Stage { #[default] Applied, Screening, Interview, Offer, Rejected, Withdrawn }
impl Stage {
    pub const ALL:[Self;6]=[Self::Applied,Self::Screening,Self::Interview,Self::Offer,Self::Rejected,Self::Withdrawn];
    pub fn label(self)->&'static str {match self {Self::Applied=>"Applied",Self::Screening=>"Screening",Self::Interview=>"Interview",Self::Offer=>"Offer",Self::Rejected=>"Rejected",Self::Withdrawn=>"Withdrawn"}}
    pub fn terminal(self)->bool {matches!(self,Self::Rejected|Self::Withdrawn)}
    pub fn change(self,next:Self,reason:&str)->Result<()> {
        ensure!(self!=next,"Choose a different application stage.");
        ensure!(!reason.trim().is_empty() && reason.trim().chars().count()<=500 && !reason.chars().any(char::is_control),"Enter a stage-change reason of 1–500 characters.");
        let allowed=if self.terminal(){next==Self::Applied}else{next.terminal() || matches!((self,next),(Self::Applied,Self::Screening|Self::Interview)|(Self::Screening,Self::Interview|Self::Offer)|(Self::Interview,Self::Offer))};
        ensure!(allowed,"This stage change is not available. Reopen a closed application at Applied.");
        Ok(())
    }
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Applicant {
    pub id:Uuid,pub company_id:Uuid,pub full_name:String,pub email:String,pub phone:String,
    pub position:String,pub source:String,pub applied_on:NaiveDate,pub notes:String,
    pub stage:Stage,pub archived:bool,pub revision:i64,
}
#[derive(Clone,Debug,Default)]
pub struct ApplicantDraft {
    pub id:Option<Uuid>,pub full_name:String,pub email:String,pub phone:String,
    pub position:String,pub source:String,pub applied_on:String,pub notes:String,
}
impl ApplicantDraft {
    pub fn new(today:NaiveDate)->Self {
        Self{id:Some(Uuid::new_v4()),applied_on:today.to_string(),..Default::default()}
    }
    pub fn fields(&self,today:NaiveDate)->Result<serde_json::Value> {
        self.validate(today)?;
        let id=self.id.context("Start an application draft before saving.")?;
        Ok(serde_json::json!({"id":id,"full_name":self.full_name.trim(),"email":self.email.trim(),"phone":self.phone.trim(),"position":self.position.trim(),"source":self.source.trim(),"applied_on":self.applied_on.trim(),"notes":self.notes.trim()}))
    }
    pub fn validate(&self,today:NaiveDate)->Result<()> {
        for (label,value,max,required) in [("Full name",&self.full_name,120,true),("Email",&self.email,254,false),("Phone",&self.phone,40,false),("Position",&self.position,100,true),("Source",&self.source,80,false)] {
            ensure!((!required || !value.trim().is_empty()) && value.trim().chars().count()<=max && !value.chars().any(char::is_control),"{label} is required or exceeds its permitted length/format.");
        }
        if !self.email.trim().is_empty(){crate::account::validate_email(&self.email)?;}
        ensure!(self.notes.trim().chars().count()<=2000 && !self.notes.chars().any(|c|c.is_control() && c!='\n'),"Notes must be at most2000 characters without unsupported control characters.");
        ensure!(parse_date(self.applied_on.trim())?<=today,"Application date cannot be in the future.");
        ensure!(self.id.is_none_or(|id|!id.is_nil()),"The applicant identifier is invalid.");
        Ok(())
    }
}
impl Applicant {
    pub fn stage_change(&self,expected_revision:i64,next:Stage,reason:&str)->Result<serde_json::Value> {
        ensure!(!self.id.is_nil() && !self.company_id.is_nil(),"The application identity is invalid.");
        ensure!(expected_revision>=0 && expected_revision==self.revision,"The application changed. Refresh and reopen it before saving.");
        ensure!(!self.archived,"Restore this application before changing its stage.");
        self.stage.change(next,reason)?;
        Ok(serde_json::json!({"id":self.id,"expected_revision":expected_revision,"stage":next,"reason":reason.trim()}))
    }
}
pub fn authorize(access:&Access,data:&Snapshot)->Result<()> {
    ensure!(data.mode!=Mode::SignedOut && access.company_id==data.company.id && access.role.manages_hr(),"Only active HR staff and Administrators can access applicant records.");
    Ok(())
}
