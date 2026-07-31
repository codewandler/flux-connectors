op discord-message-create(channel_id: String, content: String, tts: Bool) -> Any
  description "Post a message to a text channel, visible to everyone in it and notifying whoever is watching. Requires the bot to have SEND_MESSAGES in the channel. Posting twice posts two messages — Discord offers no idempotency key on this route. Discord rate-limits this route per channel and does not publish the figure: on a 429, wait the number of seconds in the `Retry-After` header before retrying, rather than retrying immediately"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://discord.com/api/v10"
  url = fmt("{base}/channels/{channel_id}/messages")
  content_type = "application/json"
  payload = { content, tts }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
