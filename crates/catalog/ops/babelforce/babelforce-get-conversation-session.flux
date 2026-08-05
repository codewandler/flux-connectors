op babelforce-get-conversation-session(conversationId: String) -> Any
  description "Get a conversation's session variables"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/{conversationId}/session")
  response = http.request(method: "GET", url)
  return response
