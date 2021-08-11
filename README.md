# Clobber

Moderation bot for matrix.

## Usage

Login interactively with `clobber --login` the first time the bot is started. If login is successful, the session is stored and the bot can subsequently be started by simply running `clobber`.

## Planned features

- [x] Matrix & bot base
  - [x] Accept invites
  - [ ] Management room
  - [ ] User management (kick/ban/mute/PL etc)
  - [ ] State handling
    - [ ] Account data
      - [ ] Protected rooms
      - [ ] Watched rooms
      - [ ] Settings
    - [ ] Room state
      - [ ] ACL management
      - [ ] Ban management
      - [ ] PL management
      - [ ] Rule list management
  - [x] Command handling
    - [ ] kick
    - [ ] ban
    - [ ] unban
    - [ ] mute
    - [ ] powerlevel
    - [ ] redact
    - [ ] help
    - [ ] status
    - [x] catch-all
  - [ ] Niceties
    - [ ] Display name
    - [ ] Avatar
    - [ ] Pretty output
- [ ] Other
  - [ ] Internationalization (?)

## TODO

- When management room is implemented, allow all members of room to invite the bot
- Perform PL checking
- Restructure configuration and initial login
- Settle on consistent style for documentation
- Refactor command handling, generate help text for commands automatically etc
