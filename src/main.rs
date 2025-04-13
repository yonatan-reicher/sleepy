use std::env;

use serenity::all::{GuildId, Message};
use serenity::async_trait;
use serenity::prelude::*;

use tokio::time::Duration;

use chrono::prelude::*;


/// Waits for the time of day given by the duration from the start of this day.
async fn wait_for_time_of_day(duration: chrono::Duration) {
    let now = Local::now();
    let time = now.date().and_hms(0, 0, 0) + duration;
    let duration_until_midnight = time - now;
    tokio::time::sleep(duration_until_midnight.to_std().unwrap()).await;
}


/// Waits for midnight.
async fn wait_for_midnight() {
    wait_for_time_of_day(chrono::Duration::hours(24)).await;
}


struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                println!("Error sending message: {:?}", why);
            }
        }
    }

    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        println!("Activated! Waiting for midnight...");

        // wait_for_midnight().await;
        // wait_for_time_of_day(chrono::Duration::hours(18) + chrono::Duration::minutes(41)).await;
        tokio::time::sleep(Duration::from_secs(90 * 60)).await;

        println!("Goodnight.");
        
        for guild in guilds {
            println!("Disconnecting in Guild: {:?}", &guild.name(&ctx.cache));
            let members = guild.members(&ctx.http, None, None).await;
            for member in members.unwrap() {
                println!("Member: {}", &member.user.name);
                let x = member.disconnect_from_voice(&ctx.http).await;
                if let Err(why) = x {
                    println!("Error disconnecting: {:?}", why);
                }
            }
        }

        println!("Disconnected all users from voice.");
        // Wait for 5 seconds before shutting down
        tokio::time::sleep(Duration::from_secs(5)).await;
        println!("Bye bye!");
        // Shutdown the client
        std::process::exit(0);
    }
}

#[tokio::main]
async fn main() {
    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified
    // about
    let intents = GatewayIntents::all();

    // Create a new instance of the Client, logging in as a bot.
    let mut client =
        Client::builder(&token, intents)
            .event_handler(Handler)
            .await
            .expect("Err creating client");

    println!("Start listening for events by starting a single shard");
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
