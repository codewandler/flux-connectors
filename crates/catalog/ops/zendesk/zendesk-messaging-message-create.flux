op zendesk-messaging-message-create(conversationId: String, author: Any, content: Any) -> Any
  description "Post Message"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/conversations/{conversationId}/messages")
  content_type = "application/json"
  payload = { author, content }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
