op babelforce-get-first-conversation-event(conversationId: String) -> Any
  description "Get a conversation's first event"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/{conversationId}/events/first")
  response = http.request(method: "GET", url)
  return response
