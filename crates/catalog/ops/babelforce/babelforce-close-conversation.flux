op babelforce-close-conversation(conversationId: String) -> Any
  description "Close a conversation"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/{conversationId}/close")
  response = http.request(method: "PUT", url)
  return response
