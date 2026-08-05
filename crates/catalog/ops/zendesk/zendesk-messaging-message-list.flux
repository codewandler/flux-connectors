op zendesk-messaging-message-list(conversationId: String) -> Any
  description "List Messages"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/conversations/{conversationId}/messages")
  response = http.request(method: "GET", url)
  return response
