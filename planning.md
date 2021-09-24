# Clobber planning document

## Events
### `sh.nao.clobber.rule`
```json
{
	"content": {
		"action": "ban",
		"entity": "@user:domain.tld",
		"reason": "spam"
	},
	"origin_server_ts": 1628599214338,
	"sender": "@mod:domain.tld",
	"state_key": "r:@user:domain.tld",
	"type": "sh.nao.clobber.list",
	"unsigned": {
		"age": 3083577327
	},
	"event_id": "$rRP4MvqZsS7lEmihlO2ltkl7cFgoR-uqXalagHW08EI",
	"room_id": "!BqCrVFYKvHgjnsFYss:domain.tld"
}
```

### `sh.nao.clobber.shortcode`
```json
{
	"type": "sh.nao.clobber.shortcode",
	"sender": "@mod:domain.tld",
	"content": {
		"shortcode": "spam"
	},
	"state_key": "",
	"origin_server_ts": 1609078000256,
	"unsigned": {
		"age": 22623721274
	},
	"event_id": "$1RNXRgcokp_895QlB6tfy6JqF98IrcaPagJ2UCtCWXg",
	"room_id": "!klUPWJCnPTTbuLGDwY:domain.tld"
}
```

## Account data
### `sh.nao.clobber.protected_rooms`
```json
{
	"type": "sh.nao.clobber.protected_rooms",
	"content": {
		"rooms": ]
			"!room1:domain.tld",
			"!room2:domain.tld"
		]
	}
}
```

### `sh.nao.clobber.watched_lists`
```json
{
	"type": "sh.nao.clobber.watched_lists",
	"content": {
		"lists": {
			"spam": {
				"room": "!room1:domain.tld",
			},
			"coc": {
				"room": "!room2:domain.tld",
			}
		}
	}
}
```
