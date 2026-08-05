op babelforce-list-prompts(page: Number, max: Number) -> Any
  description "List prompts"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/prompts")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
