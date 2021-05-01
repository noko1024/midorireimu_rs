use std::{collections::HashMap, sync::Arc};

use serenity::framework::standard::macros::hook;
use serenity::framework::standard::{
    macros::{command, group},
    CommandError, CommandResult, StandardFramework,
};
use serenity::model::{
    channel::{Channel, Message},
    gateway::Activity,
    id::*,
};
use serenity::{async_trait, framework::standard::Args};
use serenity::{
    client::{Client, Context, EventHandler},
    model::user,
};

use chrono::Local;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use useful_static::RawGlobal;

#[group]
#[commands(ping, ping2, borrow, list, giveback)]
struct General;

struct Handler;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Settings {
    admins: Vec<UserId>,
    token: String,
    globalchat_name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Rental {
    data: HashMap<String, Vec<String>>,
}

static SETTINGS: RawGlobal<Settings> = RawGlobal::new();
static RENTAL: RawGlobal<Mutex<Rental>> = RawGlobal::new();

#[tokio::main]
async fn main() {
    let mut settings = String::new();
    File::open("settings.toml")
        .await
        .unwrap()
        .read_to_string(&mut settings)
        .await
        .unwrap();
    SETTINGS.set(toml::from_str(&settings).unwrap());

    let mut rental = String::new();
    tokio::fs::File::open("rental.toml")
        .await
        .unwrap()
        .read_to_string(&mut rental)
        .await
        .unwrap();
    RENTAL.set(Mutex::new(toml::from_str(&rental).unwrap()));

    let framework = StandardFramework::new()
        .configure(|c| c.prefix(";"))
        .group(&GENERAL_GROUP)
        .after(after_hook);
    let mut client = Client::builder(&SETTINGS.token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");
    let shard = Arc::clone(&client.shard_manager);
    tokio::spawn(async move {
        if let Err(why) = client.start().await {
            println!("An error occurred while running the client: {:?}", why);
        }
    });

    tokio::signal::ctrl_c().await.unwrap();
    shard.lock().await.shutdown_all().await;
    File::create("rental.toml")
        .await
        .unwrap()
        .write_all(toml::to_string(&*RENTAL.lock().await).unwrap().as_bytes())
        .await
        .unwrap();
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(ctx, "pong").await?;
    Ok(())
}

#[command]
async fn ping2(ctx: &Context, msg: &Message) -> CommandResult {
    let now = Local::now();
    let mut msg2 = msg.channel_id.say(ctx, "計測中...").await?;
    let time = Local::now() - now;
    let send =
        String::from("pong!\n結果: **") + &time.to_std()?.as_secs_f64().to_string() + "**秒です";
    msg2.edit(ctx, |m| m.content(send)).await?;
    Ok(())
}

#[command]
async fn borrow(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let mut rental = RENTAL.lock().await;
    let userdata = rental
        .data
        .entry(msg.author.id.as_u64().to_string())
        .or_insert_with(Vec::new);
    userdata.push(args.single::<String>()?);
    msg.channel_id.say(ctx, "追加しました").await?;
    Ok(())
}

#[command]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    let rental = RENTAL.lock().await;
    let userdata = match rental.data.get(&msg.author.id.as_u64().to_string()) {
        Some(s) => s,
        None => {
            msg.channel_id
                .say(ctx, "多分まだ1度も借りたことがありませんね?")
                .await?;
            return Ok(());
        }
    };
    let mut send = String::from("あなたが借りている物のリストはこちらです\n");
    let mut counter: usize = 0;
    for i in userdata {
        counter += 1;
        send = send + &counter.to_string() + ". " + i + "\n";
    }
    msg.channel_id.say(ctx, &send).await?;
    Ok(())
}

#[command]
async fn giveback(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let mut rental = RENTAL.lock().await;
    let userdata = match rental.data.get_mut(&msg.author.id.as_u64().to_string()) {
        Some(s) => s,
        None => {
            msg.channel_id
                .say(ctx, "多分まだ1度も借りたことがありませんね?")
                .await?;
            return Ok(());
        }
    };
    let index = args.single::<usize>()? - 1;
    if index < userdata.len() {
        userdata.remove(index);
        msg.channel_id.say(ctx, "返却しました").await?;
    } else {
        msg.channel_id
            .say(
                ctx,
                "範囲外です､listコマンドでインデックスを確認してください",
            )
            .await?;
    }
    Ok(())
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: serenity::model::gateway::Ready) {
        ctx.set_activity(Activity::streaming(
            "Made by WinLinux1028 on Rust",
            "https://www.youtube.com/watch?v=otDi1f_LpOc",
        ))
        .await;
        ctx.cache.set_max_messages(51200).await;
        println!("logged in as {}", &ready.user.name);
        for i in &SETTINGS.admins {
            if let Ok(o) = i.create_dm_channel(&ctx).await {
                let _ = o.say(&ctx, "起動完了").await;
            }
        }
    }
    // async fn message_delete(
    //     &self,
    //     ctx: Context,
    //     channelid: ChannelId,
    //     deleted_message_id: MessageId,
    //     guild_id: Option<GuildId>,
    // ) {
    //     let msg = match ctx.cache.message(channelid, deleted_message_id).await {
    //         Some(s) => s,
    //         None => return,
    //     };
    //     let guild = match guild_id {
    //         Some(s) => match s.to_guild_cached(&ctx).await {
    //             Some(s) => s,
    //             None => return,
    //         },
    //         None => return,
    //     };
    //     let channel = match channelid.to_channel(&ctx).await {
    //         Ok(o) => match o {
    //             Channel::Guild(g) => g,
    //             _ => return,
    //         },
    //         Err(_) => return,
    //     };
    //     if msg.author.bot {
    //         if msg.author.id.0 == ctx.cache.current_user_id().await.0 {
    //             if &channel.name == "削除履歴" {
    //                 if msg.embeds.len() == 0 {
    //                 } else {
    //                 }
    //             }
    //         } else {
    //             return;
    //         }
    //     } else {
    //         if &channel.name == &SETTINGS.globalchat_name {
    //             return;
    //         } else {
    //         }
    //     }
    // }
}

#[hook]
async fn after_hook(ctx: &Context, msg: &Message, _: &str, error: Result<(), CommandError>) {
    match error {
        Ok(_) => {}
        Err(e) => {
            let _ = msg.channel_id.say(ctx, e).await;
        }
    }
}
