use anyhow::{Context, Result};
use mailparse::{parse_mail, ParsedMail};
use quick_xml::de::from_reader;
use serde::Deserialize;
use std::io::{Read, Cursor};
use flate2::read::GzDecoder;
use zip::ZipArchive;
use crate::db::{DmarcReport, DmarcRecord};

#[derive(Debug, Deserialize)]
struct Feedback {
    report_metadata: ReportMetadata,
    policy_published: PolicyPublished,
    record: Vec<Record>,
}

#[derive(Debug, Deserialize)]
struct ReportMetadata {
    org_name: String,
    email: String,
    report_id: String,
    date_range: DateRange,
}

#[derive(Debug, Deserialize)]
struct DateRange {
    begin: i64,
    end: i64,
}

#[derive(Debug, Deserialize)]
struct PolicyPublished {
    domain: String,
}

#[derive(Debug, Deserialize)]
struct Record {
    row: Row,
    auth_results: AuthResults,
}

#[derive(Debug, Deserialize)]
struct Row {
    source_ip: String,
    count: i64,
    policy_evaluated: PolicyEvaluated,
}

#[derive(Debug, Deserialize)]
struct PolicyEvaluated {
    disposition: String,
}

#[derive(Debug, Deserialize)]
struct AuthResults {
    dkim: Option<DkimResult>,
    spf: Option<SpfResult>,
}

#[derive(Debug, Deserialize)]
struct DkimResult {
    result: String,
}

#[derive(Debug, Deserialize)]
struct SpfResult {
    result: String,
}

pub fn parse_email(raw_email: &[u8]) -> Result<Vec<DmarcReport>> {
    let parsed = parse_mail(raw_email)?;
    let mut reports = Vec::new();
    find_and_parse_attachments(&parsed, &mut reports)?;
    Ok(reports)
}

fn find_and_parse_attachments(part: &ParsedMail, reports: &mut Vec<DmarcReport>) -> Result<()> {
    if part.subparts.is_empty() {
        let content_type = part.ctype.mimetype.clone();
        let filename = part.ctype.params.get("name").cloned().unwrap_or_default();
        
        let body = part.get_body_raw()?;
        if filename.ends_with(".zip") || content_type.contains("zip") {
            let mut archive = ZipArchive::new(Cursor::new(body))?;
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                if file.name().ends_with(".xml") {
                    let mut xml_data = Vec::new();
                    file.read_to_end(&mut xml_data)?;
                    if let Ok(report) = parse_xml(&xml_data) {
                        reports.push(report);
                    }
                }
            }
        } else if filename.ends_with(".gz") || content_type.contains("gzip") {
            let mut gz = GzDecoder::new(Cursor::new(body));
            let mut xml_data = Vec::new();
            gz.read_to_end(&mut xml_data)?;
            if let Ok(report) = parse_xml(&xml_data) {
                reports.push(report);
            }
        }
    } else {
        for sub in &part.subparts {
            find_and_parse_attachments(sub, reports)?;
        }
    }
    Ok(())
}

fn parse_xml(data: &[u8]) -> Result<DmarcReport> {
    let fb: Feedback = from_reader(data).context("Failed to parse XML")?;
    
    let mut db_records = Vec::new();
    for rec in fb.record {
        db_records.push(DmarcRecord {
            source_ip: rec.row.source_ip,
            count: rec.row.count,
            disposition: rec.row.policy_evaluated.disposition,
            dkim_result: rec.auth_results.dkim.map(|d| d.result),
            spf_result: rec.auth_results.spf.map(|s| s.result),
        });
    }

    Ok(DmarcReport {
        org_name: fb.report_metadata.org_name,
        email: fb.report_metadata.email,
        report_id: fb.report_metadata.report_id,
        begin_date: fb.report_metadata.date_range.begin,
        end_date: fb.report_metadata.date_range.end,
        domain: fb.policy_published.domain,
        records: db_records,
    })
}
