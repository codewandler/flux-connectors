op babelforce-list-all-local-automations(page: Number, max: Number) -> Any
  description "List all local automations across the account"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/automations/local")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
