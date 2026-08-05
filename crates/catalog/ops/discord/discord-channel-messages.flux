op discord-channel-messages(channel_id: String, limit: Number) -> Any
  description "Read the most recent messages in a text channel, newest first. Requires the bot to have READ_MESSAGE_HISTORY in the channel; without it Discord answers 403 rather than an empty list. The response is personal content — what people said, with author identity attached — so read it for what the calling flow needs and do not persist it beyond that"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://discord.com/api/v10"
  url = fmt("{base}/channels/{channel_id}/messages")
  response = http.request(method: "GET", query: { limit }, url)
  return response
