use axum::{body::Body, extract::Request, response::Response};
use http::StatusCode;

pub async fn handle(_req: Request<Body>) -> Response<Body> {
    let response = reqwest::get(
    "http://kbh-rss-feed.s3-website-us-east-1.amazonaws.com/byens-rum-liv--mode-range-limit.xml"
  ).await.unwrap();

    let body = response.text().await.unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/xml")
        .header("Cache-Control", "s-maxage=60, stale-while-revalidate")
        .body(Body::from(body))
        .unwrap()
}
