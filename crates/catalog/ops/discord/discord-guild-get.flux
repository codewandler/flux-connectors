op discord-guild-get(guild_id: String) -> Any
  description "Read one guild's settings and current state by id — its name, owner, moderation levels and enabled features. Membership counts are not included: they require a query flag this connector does not send, and Discord documents them as approximate"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://discord.com/api/v10"
  url = fmt("{base}/guilds/{guild_id}")
  response = http.request(method: "GET", url)
  return response
