use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    routing::get,
    Router,
};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    domains: Vec<DomainSummary>,
}

#[derive(Template)]
#[template(path = "domain.html")]
struct DomainTemplate {
    domain: String,
    reports: Vec<ReportSummary>,
    failures: Vec<FailedSource>,
}

#[derive(Template)]
#[template(path = "report.html")]
struct ReportTemplate {
    report: ReportDetail,
    records: Vec<RecordDetail>,
}

#[derive(sqlx::FromRow)]
pub struct DomainSummary {
    pub domain: String,
    pub last_report: String,
    pub total_messages: i64,
    pub failures: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
pub struct ReportSummary {
    pub id: i64,
    pub org_name: String,
    pub domain: String,
    pub begin_date: String,
    pub end_date: String,
    pub failures: i64,
}

#[derive(sqlx::FromRow)]
pub struct FailedSource {
    pub source_ip: String,
    pub count: i64,
    pub org_name: String,
    pub disposition: String,
    pub dkim: String,
    pub spf: String,
    pub report_id: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
pub struct ReportDetail {
    pub id: i64,
    pub org_name: String,
    pub email: String,
    pub domain: String,
    pub report_id: String,
}

#[derive(sqlx::FromRow)]
pub struct RecordDetail {
    pub source_ip: String,
    pub count: i64,
    pub disposition: String,
    pub dkim: String,
    pub spf: String,
}

pub fn router(pool: SqlitePool) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/domain/:domain", get(domain_detail))
        .route("/report/:id", get(report_detail))
        .with_state(Arc::new(pool))
}

async fn index(State(pool): State<Arc<SqlitePool>>) -> Html<String> {
    let domains = sqlx::query_as::<_, DomainSummary>(
        "SELECT 
            r.domain,
            datetime(MAX(r.end_date), 'unixepoch') as last_report,
            SUM(rec.count) as total_messages,
            SUM(CASE WHEN rec.disposition != 'none' OR coalesce(rec.dkim_result, 'fail') != 'pass' OR coalesce(rec.spf_result, 'fail') != 'pass' THEN rec.count ELSE 0 END) as failures
        FROM reports r
        LEFT JOIN records rec ON r.report_id = rec.report_id
        GROUP BY r.domain
        ORDER BY failures DESC, MAX(r.end_date) DESC"
    )
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let template = IndexTemplate { domains };
    Html(template.render().unwrap())
}

async fn domain_detail(
    State(pool): State<Arc<SqlitePool>>,
    Path(domain): Path<String>,
) -> Html<String> {
    let reports = sqlx::query_as::<_, ReportSummary>(
        "SELECT 
            r.id, r.org_name, r.domain, 
            datetime(r.begin_date, 'unixepoch') as begin_date, 
            datetime(r.end_date, 'unixepoch') as end_date,
            SUM(CASE WHEN rec.disposition != 'none' OR coalesce(rec.dkim_result, 'fail') != 'pass' OR coalesce(rec.spf_result, 'fail') != 'pass' THEN rec.count ELSE 0 END) as failures
         FROM reports r
         LEFT JOIN records rec ON r.report_id = rec.report_id
         WHERE r.domain = ? 
         GROUP BY r.id
         ORDER BY r.begin_date DESC"
    )
    .bind(&domain)
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let failures = sqlx::query_as::<_, FailedSource>(
        "SELECT 
            rec.source_ip, 
            rec.count, 
            r.org_name,
            rec.disposition, 
            coalesce(rec.dkim_result, 'none') as dkim, 
            coalesce(rec.spf_result, 'none') as spf,
            r.id as report_id
         FROM records rec
         JOIN reports r ON rec.report_id = r.report_id
         WHERE r.domain = ? 
           AND (rec.disposition != 'none' OR coalesce(rec.dkim_result, 'fail') != 'pass' OR coalesce(rec.spf_result, 'fail') != 'pass')
         ORDER BY rec.count DESC"
    )
    .bind(&domain)
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let template = DomainTemplate { domain, reports, failures };
    Html(template.render().unwrap())
}

async fn report_detail(
    State(pool): State<Arc<SqlitePool>>,
    Path(id): Path<i64>,
) -> Html<String> {
    let report = sqlx::query_as::<_, ReportDetail>(
        "SELECT id, org_name, email, domain, report_id FROM reports WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&*pool)
    .await
    .unwrap();

    let records = sqlx::query_as::<_, RecordDetail>(
        "SELECT source_ip, count, disposition, coalesce(dkim_result, 'none') as dkim, coalesce(spf_result, 'none') as spf FROM records WHERE report_id = ?",
    )
    .bind(&report.report_id)
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let template = ReportTemplate { report, records };
    Html(template.render().unwrap())
}
