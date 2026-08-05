op front-conversation-reply(conversation_id: String, body: String) -> Any
  description "Reply to a conversation, using its own existing recipients, channel and sender identity — this connector cannot override any of those (see providers/front.toml's header comment). Front queues delivery: the response is an acknowledgement, not the sent message, and is not visible to the recipient until Front actually delivers it. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/_error/message`, its error code at `/_error/status` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api2.frontapp.com"
  url = fmt("{base}/conversations/{conversation_id}/messages")
  content_type = "application/json"
  payload = { body }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
