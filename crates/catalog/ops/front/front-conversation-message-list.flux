op front-conversation-message-list(conversation_id: String, limit: Number) -> Any
  description "List the messages in a conversation in reverse chronological order (newest first), first page only — this connector cannot follow Front's next-page link (see providers/front.toml's header comment). Each message's `body`/`text` is the actual correspondence exchanged on this conversation. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/_error/message`, its error code at `/_error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api2.frontapp.com"
  url = fmt("{base}/conversations/{conversation_id}/messages")
  response = http.request(method: "GET", query: { limit }, url)
  return response
