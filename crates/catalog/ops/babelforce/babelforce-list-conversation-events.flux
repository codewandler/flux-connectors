op babelforce-list-conversation-events(conversationId: String) -> Any
  description "List a conversation's events"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/{conversationId}/events")
  response = http.request(method: "GET", url)
  return response
