op zendesk-messaging-message-list(conversationId: String) -> Any
  description "List messages in one conversation without exposing deep-object cursor pagination"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/conversations/{conversationId}/messages")
  response = http.request(method: "GET", url)
  return response
