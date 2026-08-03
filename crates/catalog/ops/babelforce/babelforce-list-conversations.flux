op babelforce-list-conversations(page: Number, max: Number, phone: String, state: String) -> Any
  description "List conversations"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations")
  response = http.request(method: "GET", query: { max, page, phone, state }, url)
  return response
