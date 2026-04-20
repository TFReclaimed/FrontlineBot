use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use reqwest::Client as ReqwestClient;
use reqwest::header::USER_AGENT;
use serde::{Deserialize};
use serenity::{async_trait, Client};
use serenity::all::Ready;
use serenity::gateway::ActivityData;
use serenity::prelude::*;
use tracing::{error, info};

struct Handler {
    stats_url: String,
    is_loop_running: AtomicBool,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, data_about_bot: Ready) {
        info!("{} is connected!", data_about_bot.user.name);

        let ctx = Arc::new(ctx);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            let ctx1 = Arc::clone(&ctx);

            let http_client = ReqwestClient::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Error creating HTTP client");

            let stats_url = self.stats_url.clone();

            tokio::spawn(async move {
                loop {
                    update_bot_activity(&ctx1, &http_client, &stats_url).await;
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            });

            self.is_loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ServerStats {
    online_players: u32,
    active_battles: u32,
}

async fn update_bot_activity(ctx: &Context, client: &ReqwestClient, url: &str) {
    let result = client
        .get(url)
        .header(USER_AGENT, "FrontlineBot/1.0")
        .send()
        .await;

    let response = match result {
        Ok(res) => res,
        Err(e) => {
            error!("Error fetching server stats: {:?}", e);
            ctx.set_activity(Some(ActivityData::custom("Server unreachable")));
            ctx.dnd();
            return;
        }
    };

    let stats = match response.json::<ServerStats>().await {
        Ok(stats) => stats,
        Err(e) => {
            error!("Error parsing server stats: {:?}", e);
            ctx.set_activity(Some(ActivityData::custom("Server unreachable")));
            ctx.dnd();
            return;
        }
    };

    let formatted_activity = format!("{} online | {} active battles",
                                     stats.online_players, stats.active_battles);
    ctx.set_activity(Some(ActivityData::custom(formatted_activity)));
    ctx.online();
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt::init();

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected DISCORD_TOKEN in the environment");

    let base_url = env::var("BASE_URL")
        .expect("Expected BASE_URL in the environment");

    let stats_url = format!("{}/stats.json", base_url.trim_end_matches('/'));
    info!("Using stats URL: {}", stats_url);

    let mut client = Client::builder(&token, GatewayIntents::default())
        .event_handler(Handler {
            stats_url,
            is_loop_running: AtomicBool::new(false),
        })
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
