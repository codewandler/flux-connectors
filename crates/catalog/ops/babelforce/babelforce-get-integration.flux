op babelforce-get-integration(id: String) -> Any
  description "Get an integration"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{id}")
  response = http.request(method: "GET", url)
  return response
