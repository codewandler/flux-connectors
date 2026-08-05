op babelforce-list-all-conversation-events(page: Number, max: Number) -> Any
  description "List events across all conversations"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/events")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
