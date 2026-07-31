op discord-guild-channels(guild_id: String) -> Any
  description "List every channel in a guild — text, voice, category and forum — in Discord's own ordering. This is where a channel id comes from: read `type` to find the text channels (type 0) discord-channel-messages and discord-message-create can address. Threads are not included; Discord returns them from a separate route this connector does not expose"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://discord.com/api/v10"
  url = fmt("{base}/guilds/{guild_id}/channels")
  response = http.request(method: "GET", url)
  return response
