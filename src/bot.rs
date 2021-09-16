// Clobber - a matrix moderation bot
// Copyright (C) 2020 Emelie <em@nao.sh>
// Licensed under the EUPL

//! Bot functionality.

use anyhow::anyhow;
use globset::Glob;
use matrix_sdk::{
    room::{Joined, Room},
    ruma::{
        api::client::r0::state::send_state_event::Request,
        events::{AnySyncStateEvent, EventContent},
        RoomId,
    },
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
    ruma::{serde::Raw, EventId},
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

use crate::{command, config::Config};

/// Struct for rule lists
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuleList {
    /// List shortcode
    shortcode: String,
    /// Room ID or alias
    room: RoomId,
}

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

/// Custom `EventContent` type for `sh.nao.clobber.rule` events.
#[derive(Clone, Serialize, Deserialize, Debug, EventContent)]
#[ruma_event(type = "sh.nao.clobber.rule", kind = State)]
pub struct RuleEventContent {
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
            command::handle_command(&event, &room, &client, words, &config)
                .await
                .unwrap();
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

/// Send `m.notice` reply to user.
pub async fn send_reply(
    plain: &str,
    html: &str,
    room: &Joined,
    event_id: EventId,
) -> anyhow::Result<()> {
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
) -> anyhow::Result<()> {
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
) -> anyhow::Result<()> {
    // Check `invite`, `join` and `knock` states against rule lists and apply ban if there's a match
    if event.content.membership == MembershipState::Invite
        || event.content.membership == MembershipState::Join
        || event.content.membership == MembershipState::Knock
    {
        let mut events = get_rules(client, room, event).await?;
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
) -> anyhow::Result<()> {
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

// TODO: Actually implement this properly when account data manipulation drops in matrix-sdk
/// Gets a vec of the protected `RoomId`s.
#[instrument]
pub async fn get_protected_rooms(_client: &Client) -> anyhow::Result<Vec<RoomId>> {
    Ok(vec![
        RoomId::try_from("!iYnZafYUoXkeVPOSQh:matrix.org")?,
        RoomId::try_from("!fsEJmDUHIdYFfFRTSH:jki.re")?,
    ])
}

// TODO: Actually implement this properly when account data manipulation drops in matrix-sdk
/// Creates an example room list. Placeholder until actual room lists can be fetched from account data.
pub async fn get_rule_rooms(_client: &Client) -> anyhow::Result<Vec<RoomId>> {
    Ok(vec![RoomId::try_from("!VLOvTiaFrBYAYplQFW:mozilla.org")?])
}

// There are opportunities for code reuse in this function and `get_server_rules()`, I just don't know how to do it well. A solution with generics was attempted but the cost ended up outweighing the benefits.
/// Returns a `Vec` with state events containing user rules.
#[instrument]
async fn get_rules(
    client: &Client,
    room: &Joined,
    room_member: &SyncStateEvent<MemberEventContent>,
) -> anyhow::Result<Vec<SyncStateEvent<RuleEventContent>>> {
    let mut events = Vec::new();
    for r in get_rule_rooms(client).await? {
        debug!("{:?}", r);
        let rule_room = match client.get_joined_room(&r) {
            Some(room) => room,
            None => continue,
        };
        let events_inner = rule_room
            .get_state_events(EventType::from("sh.nao.clobber.rule"))
            .await
            .unwrap();
        let events_inner: Vec<SyncStateEvent<RuleEventContent>> = events_inner
            .iter()
            .filter_map(|e| e.deserialize_as::<SyncStateEvent<RuleEventContent>>().ok())
            .filter(|e| {
                is_match_rule(
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

/// Constructs a `sh.nao.clobber.rule` state event and sends it to the appropriate room. Also
/// applies the rule to all protected rooms.
#[instrument]
pub async fn set_rule(
    client: &Client,
    rule_list: &Joined,
    entity: &str,
    action: Action,
    reason: Option<&str>,
) -> anyhow::Result<()> {
    let rule = RuleEventContent {
        entity: entity.to_string(),
        action,
        reason: reason.map(|r| r.to_string()),
    };
    let raw_rule = serde_json::value::to_raw_value(&rule)?;
    let request = Request::new_raw(
        rule_list.room_id(),
        rule.event_type(),
        entity,
        Raw::from_json(raw_rule),
    );
    client.send(request, None).await?;
    for room in get_protected_rooms(client)
        .await?
        .iter()
        .filter_map(|room_id| client.get_joined_room(room_id))
    {
        let matching_users: Vec<UserId> = room
            .joined_members()
            .await?
            .iter()
            .filter(|member| {
                is_match_entity(member.user_id().clone(), entity.to_string()).unwrap_or(false)
            })
            .map(|member| member.user_id().to_owned())
            .collect();
        for user in matching_users.iter() {
            apply_rule(&room, user.clone(), action, reason).await?;
        }
    }
    Ok(())
}

/// Gets the room for a given list. Attemps to join if not already joined to the room.
pub async fn get_list_room(client: &Client, _list: &str) -> anyhow::Result<Joined> {
    // TODO: Actually get this from account data
    let list = RuleList {
        shortcode: String::from("spam"),
        room: RoomId::try_from("!roomid:domain.tld")?,
    };
    let room = client.get_room(&list.room).unwrap();
    match room {
        Room::Joined(_) => (),
        Room::Invited(invited) => invited.accept_invitation().await?,
        _ => {
            client.join_room_by_id(&list.room).await?;
        }
    };
    Ok(client.get_joined_room(&list.room).unwrap())
}
/// Checks a `UserId` against a user rule and returns true if it matches.
pub fn is_match_rule(user_id: UserId, rule: &RuleEventContent) -> anyhow::Result<bool> {
    debug!("{:?}", rule);
    is_match_entity(user_id, rule.entity.clone())
}

/// Checks a `UserId` against a rule entity and returns true if it matches.
pub fn is_match_entity(user_id: UserId, entity: String) -> anyhow::Result<bool> {
    let glob = Glob::new(&entity)?.compile_matcher();
    let user_id = user_id;
    Ok(glob.is_match(user_id.as_str()) || glob.is_match(user_id.server_name().as_str()))
}

// TODO: Turn these into proper types and implement parsing as methods, better error handling
/// Parses a rule entity.
pub fn is_entity_server(entity: &str) -> anyhow::Result<bool> {
    if entity.starts_with('@') && entity.contains(':') && !entity.starts_with("@:") {
        Ok(false)
    } else if !entity.contains(':') {
        Ok(true)
    } else {
        Err(anyhow!("Not a valid entity"))
    }
}

#[cfg(test)]
mod tests {

    use matrix_sdk::ruma::{user_id, UserId};
    #[allow(unused_imports)]
    use tracing::{debug, error, info, warn};

    use super::{Action, RuleEventContent};
    use crate::bot::{is_entity_server, is_match_rule};

    #[tokio::test]
    #[allow(clippy::semicolon_if_nothing_returned)]
    async fn test_rule_matching() {
        let user_id: UserId = user_id!("@foobar:badserver.tld");
        let glob_user_id: UserId = user_id!("@foobar:baz.badserver.tld");

        let full = RuleEventContent {
            action: Action::Ban,
            entity: String::from("@foobar:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob = RuleEventContent {
            action: Action::Ban,
            entity: String::from("@foo*:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let full_miss = RuleEventContent {
            action: Action::Ban,
            entity: String::from("@bob:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob_miss = RuleEventContent {
            action: Action::Ban,
            entity: String::from("@*bob:badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let full_server = RuleEventContent {
            action: Action::Ban,
            entity: String::from("badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob_subdomain = RuleEventContent {
            action: Action::Ban,
            entity: String::from("*.badserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob_server = RuleEventContent {
            action: Action::Ban,
            entity: String::from("*adserver.tld"),
            reason: Some(String::from("spam")),
        };

        let full_server_miss = RuleEventContent {
            action: Action::Ban,
            entity: String::from("goodserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob_subdomain_miss = RuleEventContent {
            action: Action::Ban,
            entity: String::from("*.goodserver.tld"),
            reason: Some(String::from("spam")),
        };

        let glob_server_miss = RuleEventContent {
            action: Action::Ban,
            entity: String::from("*goodserver.tld"),
            reason: Some(String::from("spam")),
        };
        assert!(is_match_rule(user_id.clone(), &full).unwrap());
        assert!(is_match_rule(user_id.clone(), &glob).unwrap());
        assert!(!is_match_rule(user_id.clone(), &full_miss).unwrap());
        assert!(!is_match_rule(user_id.clone(), &glob_miss).unwrap());
        assert!(is_match_rule(user_id.clone(), &full_server).unwrap());
        assert!(is_match_rule(glob_user_id.clone(), &glob_subdomain).unwrap());
        assert!(is_match_rule(user_id.clone(), &glob_server).unwrap());
        assert!(!is_match_rule(user_id.clone(), &full_server_miss).unwrap());
        assert!(!is_match_rule(user_id, &glob_server_miss).unwrap());
        assert!(!is_match_rule(glob_user_id, &glob_subdomain_miss).unwrap());
    }

    #[test]
    #[allow(clippy::semicolon_if_nothing_returned)]
    fn test_entity_parsing() -> anyhow::Result<()> {
        let user1 = "@user:domain.tld";
        let user2 = "@*:domain.tld";
        let user3 = "@user*:domain.tld";
        let user4 = "@user:*.domain.tld";

        let server1 = "domain.tld";
        let server2 = "*.domain.tld";
        let server3 = "domain*.tld";
        let server4 = "*.tld";
        let server5 = "tld";

        let invalid1 = "user:domain.tld";
        let invalid2 = "domain.tld:8448";

        assert!(!is_entity_server(user1)?);
        assert!(!is_entity_server(user2)?);
        assert!(!is_entity_server(user3)?);
        assert!(!is_entity_server(user4)?);

        assert!(is_entity_server(server1)?);
        assert!(is_entity_server(server2)?);
        assert!(is_entity_server(server3)?);
        assert!(is_entity_server(server4)?);
        assert!(is_entity_server(server5)?);

        assert_eq!(
            is_entity_server(invalid1).unwrap_err().to_string(),
            "Not a valid entity"
        );
        assert_eq!(
            is_entity_server(invalid2).unwrap_err().to_string(),
            "Not a valid entity"
        );
        Ok(())
    }
}
