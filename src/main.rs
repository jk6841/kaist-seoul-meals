use anyhow::{bail, Context, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use std::env;
use std::time::Duration;
use tracing::{error, info, Level};
use tracing_subscriber::fmt::format;
use tracing_subscriber::FmtSubscriber;

const MSG_NO_MEALS: &str = "메뉴가 없어요!";
const TITLE_LUNCH: &str = "오늘의 점심";
const TITLE_DINNER: &str = "오늘의 저녁";
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .event_format(format().json())
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("job 시작");

    if let Err(e) = run().await {
        // {:#}: 원인 체인까지 한 줄로 출력
        error!("job 실패: {e:#}");
        std::process::exit(1);
    }

    info!("job 완료");
}

async fn run() -> Result<()> {
    dotenvy::dotenv().ok();

    let kaist_meals_url = get_env("KAIST_MEALS_URL")?;
    let dooray_webhook_url = get_env("DOORAY_WEBHOOK_URL")?;

    let meals = fetch_meals(&kaist_meals_url).await?;

    send_dooray_webhook(&dooray_webhook_url, &kaist_meals_url, &meals).await
}

async fn fetch_meals(url: &str) -> Result<Vec<String>> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(HTTP_TIMEOUT)
        .build()?;

    let html = client
        .get(url)
        .send()
        .await
        .context("학식 페이지 요청 실패")?
        .error_for_status()
        .context("학식 페이지 응답 오류")?
        .text()
        .await
        .context("학식 페이지 본문 읽기 실패")?;

    let document = Html::parse_document(&html);

    let td_selector = Selector::parse("td").expect("td 선택자는 항상 유효함");
    let tds: Vec<_> = document.select(&td_selector).collect();

    // 요소가 없으면 페이지 구조가 바뀐 것이므로 실패 처리.
    // 요소는 있는데 내용만 비어 있으면 (공휴일 등) 안내 문구를 보냄.
    (0..=2)
        .map(|idx| {
            let element = tds
                .get(idx)
                .with_context(|| format!("인덱스 {idx}의 td 요소를 찾을 수 없습니다. 페이지 구조가 바뀌었을 수 있습니다."))?;

            let text = element.text().collect::<Vec<_>>().join(" ");
            let formatted_menu = format_menu(text.trim());

            Ok(if formatted_menu.is_empty() {
                MSG_NO_MEALS.to_string()
            } else {
                formatted_menu
            })
        })
        .collect()
}

async fn send_dooray_webhook(webhook_url: &str, kaist_meals_url: &str, meals: &Vec<String>) -> Result<()> {
    let client = Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()?;
    let lunch = &meals[1];
    let dinner = &meals[2];

    let payload = serde_json::json!({
        "botName": "KAIST 서울캠 학식",
        "botIconImage": "https://www.kaist.ac.kr/favicon.ico",
        "attachments": [
            {
                "title": TITLE_LUNCH.to_string(),
                "titleLink": kaist_meals_url,
                "text": lunch,
                "color": "red",
            },
            {
                "title": TITLE_DINNER.to_string(),
                "titleLink": kaist_meals_url,
                "text": dinner,
                "color": "blue",
            },
        ]
    });

    let res = client
        .post(webhook_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .context("웹훅 요청 실패")?;

    let status = res.status();

    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        bail!("웹훅 전송 실패: {status} - {text}");
    }

    info!("웹훅 전송 성공");
    Ok(())
}

fn format_menu(text: &str) -> String {
    let mut result = String::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // 빈 줄은 그대로 유지 (구분용)
        if trimmed.is_empty() {
            result.push('\n');
            continue;
        }

        if trimmed.ends_with("층") {
            result.push_str(trimmed);
            result.push('\n');

        } else if trimmed.ends_with("코너") {
            result.push_str("- ");
            result.push_str(trimmed);
            result.push('\n');

        } else {
            result.push_str("- ");
            result.push_str(trimmed);
            result.push('\n');

        }

    }

    result.trim_end().to_string()
}

fn get_env(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("{key} 환경변수가 설정되어 있지 않습니다."))
}