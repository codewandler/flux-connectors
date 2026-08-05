op zendesk-messaging-conversation-get(conversationId: String) -> Any
  description "Get one conversation by id; this bounded read is the Messaging service's diagnostic operation"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/conversations/{conversationId}")
  response = http.request(method: "GET", url)
  return response
