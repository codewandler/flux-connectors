op discord-guild-list -> Any
  description "List the guilds (servers) this bot has been installed into — the complete set of guild ids the rest of this connector can address. Returns a partial guild object per entry, not the full one: call discord-guild-get for a guild's settings. Discord returns at most 200 per page and this connector does not page, so a bot in more guilds than that sees only the first 200"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://discord.com/api/v10"
  url = fmt("{base}/users/@me/guilds")
  response = http.request(method: "GET", url)
  return response
