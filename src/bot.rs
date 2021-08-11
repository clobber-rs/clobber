// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! Bot functionality, command handling, etc.

use anyhow::{anyhow, Error};
use globset::Glob;
use matrix_sdk::{
    room::{Joined, Room},
    ruma::EventId,
    ruma::{events::AnySyncStateEvent, RoomId},
    ruma::{
        events::{
            macros::EventContent,
            room::{
                member::{MemberEventContent, MembershipState},
                message::{
                    InReplyTo, MessageEventContent, MessageType, Relation, TextMessageEventContent,
                },
            },
            AnyMessageEventContent, AnyStateEventContent, EventType, StrippedStateEvent,
            SyncMessageEvent, SyncStateEvent,
        },
        int, UserId,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use serde_with::rust::string_empty_as_none;
use std::convert::TryFrom;
use std::time::Duration;
use tokio::time::sleep;
use tracing_attributes::instrument;

#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

use crate::config::Config;

/// Enum of available actions to apply to entity that matches rules.
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize)]
pub enum Action {
    /// Ban the entity.
    Ban,
    /// Kick the entity.
    Kick,
    /// Mute the entity.
    Mute,
}

/// Custom `EventContent` type for `sh.nao.clibber.rule.user` events.
#[derive(Clone, Serialize, Deserialize, Debug, EventContent)]
#[ruma_event(type = "sh.nao.clobber.rule.user", kind = State)]
pub struct UserRuleEventContent {
    /// The user(s) the rule applies to.
    entity: String,
    /// The action to take on successful match.
    action: Action,
    /// User-supplied reason for creating the rule.
    #[serde(with = "string_empty_as_none")]
    reason: Option<String>,
}

/// Custom `EventContent` type for `sh.nao.clibber.rule.server` events.
#[derive(Clone, Serialize, Deserialize, Debug, EventContent)]
#[ruma_event(type = "sh.nao.clobber.rule.server", kind = State)]
pub struct ServerRuleEventContent {
    /// The user(s) the rule applies to.
    entity: String,
    /// The action to take on successful match.
    action: Action,
    /// User-supplied reason for creating the rule.
    #[serde(with = "string_empty_as_none")]
    reason: Option<String>,
}

#[instrument]
pub async fn on_room_message(
    event: SyncMessageEvent<MessageEventContent>,
    room: Room,
    client: Client,
    config: Config,
) {
    if let Room::Joined(room) = room {
        // Match on m.text messages and get the message body
        let msg_body = if let SyncMessageEvent {
            content:
                MessageEventContent {
                    msgtype: MessageType::Text(TextMessageEventContent { body: msg_body, .. }),
                    ..
                },
            ..
        } = &event
        {
            info!("Matching on received message");
            msg_body
        } else {
            info!("Not matching on received message");
            return;
        };
        if msg_body
            .trim_start()
            .starts_with(&config.bot.command_prefix.to_string())
        {
            let mut words: Vec<&str> = msg_body.split(' ').collect();
            // Split prefix into separate word if 1 char long. don't judge ;-;
            if config.bot.command_prefix.chars().count() == 1 {
                words[0] = &words[0][1..];
                words.insert(0, &config.bot.command_prefix);
            }
            info!("Running command: {:?}", words);
            handle_command(&event, &room, words, &config).await.unwrap();
        }
    }
}

#[instrument]
pub async fn on_stripped_state_member(
    event: StrippedStateEvent<MemberEventContent>,
    room: Room,
    client: Client,
    config: Config,
) {
    // If `m.member` event is an invite and the bot is the invitee
    if event.content.membership == MembershipState::Invite
        && event.state_key == client.user_id().await.unwrap()
    {
        accept_invite(&event, &room, &client, &config)
            .await
            .unwrap();
    }
}

#[instrument]
pub async fn on_room_member(event: SyncStateEvent<MemberEventContent>, room: Room, client: Client) {
    if let Room::Joined(room) = room {
        info!("Event handler firing");
        check_member(&event, &room, &client).await.unwrap();
    };
}

/// Handles incoming commands and dispatches relevant functions.
#[instrument]
async fn handle_command(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    commands: Vec<&str>,
    config: &Config,
) -> Result<(), anyhow::Error> {
    if commands.len() < 2 {
        command_help(event, room).await?;
        return Ok(());
    }
    let base_command = commands[1];
    let _arguments = &commands[2..];
    match base_command {
        "help" => command_help(event, room).await?,
        _ => command_unknown(event, room, config).await?,
    }
    Ok(())
}

/// Fallback when an unrecognized command is invoked.
#[instrument]
async fn command_unknown(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
    config: &Config,
) -> Result<(), anyhow::Error> {
    send_reply(
        &format!("Unrecognized command, please try again or see {} help for available commands.", &config.bot.command_prefix),
        &format!("Unrecognized command, please try again or see <code>{} help</code> for available commands.", &config.bot.command_prefix),
        room,
        event.event_id.clone(),
    ).await?;
    Ok(())
}

/// Send help information.
#[instrument]
async fn command_help(
    event: &SyncMessageEvent<MessageEventContent>,
    room: &Joined,
) -> Result<(), anyhow::Error> {
    let message = "There's nothing here yet";
    send_reply(message, message, room, event.event_id.clone()).await?;
    Ok(())
}

/// Send `m.notice` reply to user.
async fn send_reply(
    plain: &str,
    html: &str,
    room: &Joined,
    event_id: EventId,
) -> Result<(), anyhow::Error> {
    let mut notice = MessageEventContent::notice_html(plain, html);
    notice.relates_to = Some(Relation::Reply {
        in_reply_to: InReplyTo::new(event_id),
    });
    let content = AnyMessageEventContent::RoomMessage(notice);
    room.send(content, None).await?;
    Ok(())
}

// Inspired by the AutoJoin example in matrix-rust-sdk
/// Handles incoming invites.
#[instrument]
async fn accept_invite(
    event: &StrippedStateEvent<MemberEventContent>,
    room: &Room,
    client: &Client,
    config: &Config,
) -> Result<(), anyhow::Error> {
    if let Room::Invited(room) = room {
        if !config
            .bot
            .allow_invites
            .iter()
            .any(|user| user == &event.sender)
        {
            info!(
                "Unauthorized user {} tried to invite bot to {}",
                &event.sender,
                &room.room_id()
            );
            return Ok(());
        }
        // Keep trying to join the room we've been invited with exponential backoff until an hour
        // has passed.
        debug!("Joining room: {}", room.room_id());
        let mut delay = 2;
        while let Err(e) = room.accept_invitation().await {
            warn!(
                "Failed to join room: {} ({:?}), retrying in {}s",
                room.room_id(),
                e,
                delay
            );

            sleep(Duration::from_secs(delay)).await;
            delay *= 2;
            if delay > 3600 {
                error!("Couldn't join room {} ({:?})", room.room_id(), e);
                break;
            }
        }
        info!("Joined room: {}", room.room_id());
    }
    Ok(())
}

/// Called on `m.room.member` events. Checks whether the user matches any of the rules, and calls
/// `apply_rule()` if they do.
#[instrument]
async fn check_member(
    event: &SyncStateEvent<MemberEventContent>,
    room: &Joined,
    client: &Client,
) -> Result<(), anyhow::Error> {
    // Check `invite`, `join` and `knock` states against rule lists and apply ban if there's a match
    if event.content.membership == MembershipState::Invite
        || event.content.membership == MembershipState::Join
        || event.content.membership == MembershipState::Knock
    {
        let mut events = get_user_rules(client, room, event).await?;
        events.sort_by_key(|e| e.content.action);
        if let Some(event) = events.first() {
            apply_rule(
                room,
                UserId::try_from(event.state_key.clone())?,
                event.content.action,
                event.content.reason.as_deref(),
            )
            .await?;
        };

        let mut events = get_server_rules(client, room, event).await?;
        events.sort_by_key(|e| e.content.action);
        if let Some(event) = events.first() {
            apply_rule(
                room,
                UserId::try_from(event.state_key.clone())?,
                event.content.action,
                event.content.reason.as_deref(),
            )
            .await?;
        };
    }
    Ok(())
}

/// Apply a moderation action to a user based on the rule. The action taken is ordered by severity, decided by the definition order of the `Action` enum.
async fn apply_rule(
    room: &Joined,
    user: UserId,
    action: Action,
    reason: Option<&str>,
) -> Result<(), anyhow::Error> {
    // TODO: Write tests for these actions
    match action {
        Action::Ban => {
            room.ban_user(&user, reason).await?;
        }
        Action::Kick => {
            room.kick_user(&user, reason).await?;
        }
        Action::Mute => {
            let powerlevel = room
                .get_state_event(EventType::RoomPowerLevels, "")
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "There is no `m.room.power_levels` event in {}! Something is very wrong.",
                        room.room_id()
                    )
                })?
                .deserialize()?;
            let mut powerlevel = match powerlevel {
                AnySyncStateEvent::RoomPowerLevels(e) => e.content,
                _ => {
                    return Err(anyhow!(
                        "Incorrect event type returned by `Joined::get_state_event()`"
                    ))
                }
            };
            powerlevel.users.insert(user, int!(-1));
            room.send_state_event(AnyStateEventContent::RoomPowerLevels(powerlevel), "")
                .await?;
        }
    };
    Ok(())
}

// There are opportunities for code reuse in this function and `get_server_rules()`, I just don't know how to do it well. A solution with generics was attempted but the cost ended up outweighing the benefits.
/// Returns a `Vec` with state events containing user rules.
#[instrument]
async fn get_user_rules(
    client: &Client,
    room: &Joined,
    room_member: &SyncStateEvent<MemberEventContent>,
) -> Result<Vec<SyncStateEvent<UserRuleEventContent>>, Error> {
    let mut events = Vec::new();
    for r in mock_room_list() {
        debug!("{:?}", r);
        let rule_room = match client.get_joined_room(&r) {
            Some(room) => room,
            None => continue,
        };
        let events_inner = rule_room
            .get_state_events(EventType::from("sh.nao.clobber.rule.user"))
            .await
            .unwrap();
        let events_inner: Vec<SyncStateEvent<UserRuleEventContent>> = events_inner
            .iter()
            .filter_map(|e| {
                e.deserialize_as::<SyncStateEvent<UserRuleEventContent>>()
                    .ok()
            })
            .filter(|e| {
                is_match_user_rule(
                    UserId::try_from(room_member.state_key.clone()).unwrap(),
                    &e.content,
                )
                .unwrap()
            })
            .collect();
        events.extend(events_inner);
    }
    Ok(events)
}

/// Returns a `Vec` with state events containing server rules.
#[instrument]
async fn get_server_rules(
    client: &Client,
    room: &Joined,
    room_member: &SyncStateEvent<MemberEventContent>,
) -> Result<Vec<SyncStateEvent<ServerRuleEventContent>>, Error> {
    let mut events = Vec::new();
    for r in mock_room_list() {
        debug!("{:?}", r);
        let rule_room = match client.get_joined_room(&r) {
            Some(room) => room,
            None => continue,
        };
        let events_inner = rule_room
            .get_state_events(EventType::from("sh.nao.clobber.rule.server"))
            .await
            .unwrap();
        let events_inner: Vec<SyncStateEvent<ServerRuleEventContent>> = events_inner
            .iter()
            .filter_map(|e| {
                e.deserialize_as::<SyncStateEvent<ServerRuleEventContent>>()
                    .ok()
            })
            .filter(|e| {
                is_match_server_rule(
                    UserId::try_from(room_member.state_key.clone()).unwrap(),
                    &e.content,
                )
            })
            .collect();
        events.extend(events_inner);
    }
    Ok(events)
}

/// Checks a `UserId` against a user rule and returns true if it matches.
fn is_match_user_rule(user_id: UserId, rule: &UserRuleEventContent) -> Result<bool, anyhow::Error> {
    debug!("{:?}", rule);
    let glob = Glob::new(&rule.entity)?.compile_matcher();
    let user_id = user_id;
    Ok(glob.is_match(user_id.as_str()))
}

/// Checks a `UserId` against a server rule and returns true if it matches.
fn is_match_server_rule(user_id: UserId, rule: &ServerRuleEventContent) -> bool {
    debug!("{:?}", rule);
    let glob = Glob::new(&rule.entity).unwrap().compile_matcher();
    let user_id = user_id;
    glob.is_match(user_id.server_name().as_str())
}

/// Creates an example room list. Placeholder until actual room lists can be fetched from account data.
fn mock_room_list() -> Vec<RoomId> {
    vec![RoomId::try_from("!BqCrVFYKvHgjnsFYss:queersin.space").unwrap()]
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use matrix_sdk::ruma::UserId;
    #[allow(unused_imports)]
    use tracing::{debug, error, info, warn};

    use super::{
        is_match_server_rule, is_match_user_rule, Action, ServerRuleEventContent,
        UserRuleEventContent,
    };

    #[tokio::test]
    #[allow(clippy::semicolon_if_nothing_returned)]
    async fn test_server_rules() {
        let baduser1: UserId = UserId::try_from("@foobar:badserver.tld").unwrap();
        let baduser2: UserId = UserId::try_from("@foobar:subdomain.badserver.tld").unwrap();

        let full = ServerRuleEventContent {
            action: Action::Ban,
            entity: String::from("badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let tld = ServerRuleEventContent {
            action: Action::Ban,
            entity: String::from("*.tld"),
            reason: Some(String::from("spam")),
        };

        let full_miss = ServerRuleEventContent {
            action: Action::Ban,
            entity: String::from("goodserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob_miss = ServerRuleEventContent {
            action: Action::Ban,
            entity: String::from("*.goodserver.tld"),
            reason: Some(String::from("spam")),
        };

        let tld_miss = ServerRuleEventContent {
            action: Action::Ban,
            entity: String::from("*.xyz"),
            reason: Some(String::from("spam")),
        };

        assert!(is_match_server_rule(baduser1.clone(), &full));
        assert!(is_match_server_rule(baduser1.clone(), &tld));

        assert!(!is_match_server_rule(baduser1.clone(), &full_miss));
        assert!(!is_match_server_rule(baduser1.clone(), &glob_miss));
        assert!(!is_match_server_rule(baduser1, &tld_miss));

        let glob = ServerRuleEventContent {
            action: Action::Ban,
            entity: String::from("*.badserver.tld"),
            reason: Some(String::from("spam")),
        };

        assert!(is_match_server_rule(baduser2, &glob));
    }

    #[tokio::test]
    #[allow(clippy::semicolon_if_nothing_returned)]
    async fn test_user_rules() {
        let user_id: UserId = UserId::try_from("@foobar:badserver.tld").unwrap();

        let full = UserRuleEventContent {
            action: Action::Ban,
            entity: String::from("@foobar:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob = UserRuleEventContent {
            action: Action::Ban,
            entity: String::from("@foo*:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let full_miss = UserRuleEventContent {
            action: Action::Ban,
            entity: String::from("@bob:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob_miss = UserRuleEventContent {
            action: Action::Ban,
            entity: String::from("@*bob:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        assert!(is_match_user_rule(user_id.clone(), &full).unwrap());
        assert!(is_match_user_rule(user_id.clone(), &glob).unwrap());
        assert!(!is_match_user_rule(user_id.clone(), &full_miss).unwrap());
        assert!(!is_match_user_rule(user_id, &glob_miss).unwrap());
    }
}
