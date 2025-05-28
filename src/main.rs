use std::env;
use std::sync::Arc;

use serenity::all::{ChannelId, Error, GuildId, Message};
use serenity::async_trait;
use serenity::prelude::*;

use tokio::time::Duration;

/*
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
*/

#[derive(Clone, Debug)]
enum Log {
    Start(Duration),
    Disconnecting,
    InvalidGuildId(GuildId),
    ErrorDisconnectingGuild(Arc<Error>),
    ErrorDisconnectingMember(String, Arc<Error>),
    DoneDisconnecting,
}

impl Log {
    pub fn to_message(&self) -> String {
        use Log::*;

        match self {
            Start(duration) => format!(
                "Everyone will be disconnected in {} minutes",
                duration.as_secs() / 60
            ),
            InvalidGuildId(guild_id) => format!("Invalid server ID: {}", guild_id),
            ErrorDisconnectingGuild(e) => format!("Error disconnecting from server: {}", e),
            ErrorDisconnectingMember(m, e) => format!("Error disconnecting member {}: {}", m, e),
            Disconnecting => "Disconnecting everyone...".to_string(),
            DoneDisconnecting => "Done!".to_string(),
        }
    }
}

async fn guild_goes_to_sleep<L: AsyncFnMut(Log)>(
    ctx: &Context,
    guild: GuildId,
    delay: Duration,
    mut logger: L,
) {
    use Log::*;

    logger(Start(delay)).await;
    tokio::time::sleep(delay).await;

    let Some(_) = guild.name(&ctx.cache) else {
        logger(InvalidGuildId(guild)).await;
        return;
    };
    logger(Disconnecting).await;
    let members = guild.members(&ctx.http, None, None).await;
    match members {
        Err(e) => logger(ErrorDisconnectingGuild(e.into())).await,
        Ok(members) => {
            for member in members {
                let name = member.user.name.clone();
                match member.disconnect_from_voice(&ctx.http).await {
                    Ok(_) => (),
                    Err(e) => logger(ErrorDisconnectingMember(name, e.into())).await,
                }
            }
        }
    }
    logger(DoneDisconnecting).await;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Command {
    Help,
    Ping,
    Sleep { delay: Duration },
}

impl Command {
    pub fn help() -> &'static [&'static str] {
        &[
            "!ping - Responds with Pong!",
            "!sleep [<delay>] - Disconnects all members from voice channels after the given delay (minutes). Default delay is 90 minutes.",
            "!help - Shows this help message!",
        ]
    }

    pub fn parse(s: &str) -> Result<Option<Self>, String> {
        let s = s.trim();
        if !s.starts_with('!') {
            return Ok(None);
        }
        let tokens = s.split_whitespace().collect::<Vec<_>>();
        match tokens.as_slice() {
            ["!help"] => Ok(Some(Command::Help)),
            ["!ping"] => Ok(Some(Command::Ping)),
            ["!sleep"] => Ok(Some(Command::Sleep {
                delay: Duration::from_secs(90 * 60),
            })),
            ["!sleep", delay] => {
                if let Ok(delay) = delay.parse::<u64>() {
                    Ok(Some(Command::Sleep {
                        delay: Duration::from_secs(delay * 60),
                    }))
                } else {
                    Err(format!("Invalid !sleep delay: {}. Must be a number", delay))
                }
            }
            _ => Err(format!("Unknown command: {}", s)),
        }
    }
}

trait Env {
    fn ctx(&self) -> &Arc<Context>;
    fn channel_id(&self) -> ChannelId;
    fn guild_id(&self) -> GuildId;
    fn log_error(&mut self, msg: &str);
}

async fn say<E: Env>(msg: &str, env: &mut E) {
    match env.channel_id().say(&env.ctx().http, msg).await {
        Ok(_) => (),
        Err(e) => {
            env.log_error(&format!("Error sending message: {}", e));
        }
    }
}

async fn run_command<E: Env>(c: &Command, mut env: E) {
    use Command::*;

    match c {
        Help => {
            let help = Command::help().join("\n");
            say(&help, &mut env).await;
        }
        Ping => say("Pong!", &mut env).await,
        Sleep { delay } => {
            guild_goes_to_sleep(&env.ctx().clone(), env.guild_id(), *delay, async move |e| {
                let m = e.to_message();
                say(&m, &mut env).await;
            })
            .await;
        }
    }
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot { return; }

        struct E {
            ctx: Arc<Context>,
            channel_id: ChannelId,
            guild_id: GuildId,
        }
        impl Env for E {
            fn ctx(&self) -> &Arc<Context> {
                &self.ctx
            }
            fn channel_id(&self) -> ChannelId {
                self.channel_id
            }
            fn guild_id(&self) -> GuildId {
                self.guild_id
            }
            fn log_error(&mut self, msg: &str) {
                println!("Error: {}", msg);
            }
        }
        let mut env = E {
            ctx: ctx.into(),
            channel_id: msg.channel_id,
            guild_id: msg.guild_id.unwrap(),
        };
        let command = match Command::parse(&msg.content) {
            Ok(Some(c)) => c,
            Ok(None) => return, // Not for us, just ignore
            Err(e) => {
                say(&format!("Error: {}", e), &mut env).await;
                return;
            }
        };
        run_command(&command, env).await;
    }

    /*
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
    */
}

#[tokio::main]
async fn main() {
    // Login with a bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified
    // about
    let intents = GatewayIntents::all();

    // Create a new instance of the Client, logging in as a bot.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    println!("Start listening for events by starting a single shard");
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
