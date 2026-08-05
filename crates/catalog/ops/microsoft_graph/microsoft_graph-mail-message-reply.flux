op microsoft_graph-mail-message-reply(message_id: String, comment: String) -> Any
  description "Reply to a message. Graph resolves the recipients automatically — the original message's `replyTo` if it specifies one, otherwise its `from` — and this operation cannot override, widen or add to that audience (C-56 excludes the `message` override Graph's own reply action accepts). Delivered within seconds and cannot be recalled. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/messages/{message_id}/reply")
  content_type = "application/json"
  payload = { comment }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
