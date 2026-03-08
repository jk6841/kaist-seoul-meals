use anyhow::{Context, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use std::env;
use tracing::{error, info, Level};
use tracing_subscriber::fmt::format;
use tracing_subscriber::FmtSubscriber;

const MSG_NO_MEALS: &str = "메뉴가 없어요!";

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .event_format(format().json())
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("job 시작");

    dotenvy::dotenv().ok();

    let kaist_meals_url = get_env("KAIST_MEALS_URL");
    let dooray_webhook_url = get_env("DOORAY_WEBHOOK_URL");

    let meals = fetch_meals(&kaist_meals_url).await;

    send_dooray_webhook(&dooray_webhook_url, &kaist_meals_url, &meals).await?;

    info!("job 완료");
    Ok(())
}

async fn fetch_meals(url: &str) -> Vec<String> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build().unwrap();

    let html = client
        .get(url)
        .send()
        .await.unwrap()
        .error_for_status().unwrap()
        .text()
        .await.unwrap();

    let document = Html::parse_document(&html);

    let td_selector = Selector::parse("td").unwrap();
    let tds: Vec<_> = document.select(&td_selector).collect();

    let results: Vec<String> = (0..=2)
        .map(|idx| {
            tds.get(idx)
                .context(format!("인덱스 {idx}의 요소를 찾을 수 없습니다."))
                .map(|element| {
                    element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string()
                })
                .map(|element| {
                    let formatted_menu = format_menu(&element);
                    if formatted_menu.is_empty() {
                        MSG_NO_MEALS.to_string()
                    } else {
                        formatted_menu
                    }
                })
                .unwrap_or_else(|_| "".to_string())
        })
        .collect();

    results
}

async fn send_dooray_webhook(webhook_url: &str, kaist_meals_url: &str, meals: &Vec<String>) -> Result<()> {
    let client = Client::new();
    let lunch = &meals[1];
    let dinner = &meals[2];

    let payload = serde_json::json!({
        "botName": "KAIST 서울캠 학식",
        "botIconImage": "https://www.kaist.ac.kr/favicon.ico",
        "attachments": [
            {
                "title": "오늘의 점심".to_string(),
                "titleLink": kaist_meals_url,
                "text": lunch,
                "color": "red",
            },
            {
                "title": "오늘의 저녁".to_string(),
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
        .await?;

    let status = res.status();

    if status.is_success() {
        info!("웹훅 전송 성공");
    } else {
        let text = res.text().await?;
        error!("웹훅 전송 실패: {} - {}", status, text);
    }

    Ok(())
}

fn format_menu(text: &String) -> String {
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

fn get_env(key: &str) -> String {
    env::var(key)
        .context(format!("{key} 환경변수가 설정되어 있지 않습니다.")).unwrap()
}