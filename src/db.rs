use anyhow::Result;
use sqlx::SqlitePool;

pub struct DmarcReport {
    pub org_name: String,
    pub email: String,
    pub report_id: String,
    pub begin_date: i64,
    pub end_date: i64,
    pub domain: String,
    pub records: Vec<DmarcRecord>,
}

pub struct DmarcRecord {
    pub source_ip: String,
    pub count: i64,
    pub disposition: String,
    pub dkim_result: Option<String>,
    pub spf_result: Option<String>,
}

pub async fn save_report(pool: &SqlitePool, report: DmarcReport) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO reports (org_name, email, report_id, begin_date, end_date, domain)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(report_id) DO NOTHING"
    )
    .bind(&report.org_name)
    .bind(&report.email)
    .bind(&report.report_id)
    .bind(report.begin_date)
    .bind(report.end_date)
    .bind(&report.domain)
    .execute(&mut *tx)
    .await?;

    for rec in report.records {
        sqlx::query(
            "INSERT INTO records (report_id, source_ip, count, disposition, dkim_result, spf_result)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&report.report_id)
        .bind(&rec.source_ip)
        .bind(rec.count)
        .bind(&rec.disposition)
        .bind(&rec.dkim_result)
        .bind(&rec.spf_result)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
