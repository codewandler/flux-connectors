op babelforce-list-integration-tokens(id: String) -> Any
  description "List integration tokens"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{id}/tokens")
  response = http.request(method: "GET", url)
  return response
