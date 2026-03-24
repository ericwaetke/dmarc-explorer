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
    pub last_report: i64,
    pub total_messages: i64,
    pub failures: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
pub struct ReportSummary {
    pub id: i64,
    pub org_name: String,
    pub domain: String,
    pub begin_date: i64,
    pub end_date: i64,
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
            MAX(r.end_date) as last_report,
            SUM(rec.count) as total_messages,
            SUM(CASE WHEN rec.disposition != 'none' OR coalesce(rec.dkim_result, 'fail') != 'pass' OR coalesce(rec.spf_result, 'fail') != 'pass' THEN rec.count ELSE 0 END) as failures
        FROM reports r
        LEFT JOIN records rec ON r.report_id = rec.report_id
        GROUP BY r.domain
        ORDER BY failures DESC, last_report DESC"
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
        "SELECT id, org_name, domain, begin_date, end_date FROM reports WHERE domain = ? ORDER BY begin_date DESC"
    )
    .bind(&domain)
    .fetch_all(&*pool)
    .await
    .unwrap_or_default();

    let template = DomainTemplate { domain, reports };
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
