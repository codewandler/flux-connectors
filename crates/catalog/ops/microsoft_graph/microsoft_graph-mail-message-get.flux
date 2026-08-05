op microsoft_graph-mail-message-get(message_id: String) -> Any
  description "Get one Outlook message by id: subject, sender, recipients, timestamps and the body — HTML by default. This is personal correspondence; treat the sender, recipients and body as personal data. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/messages/{message_id}")
  response = http.request(method: "GET", url)
  return response
