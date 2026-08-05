op babelforce-open-conversation(conversationId: String) -> Any
  description "Reopen a conversation"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/{conversationId}/open")
  response = http.request(method: "PUT", url)
  return response
