op babelforce-get-conversation(id: String) -> Any
  description "Get a conversation"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/{id}")
  response = http.request(method: "GET", url)
  return response
