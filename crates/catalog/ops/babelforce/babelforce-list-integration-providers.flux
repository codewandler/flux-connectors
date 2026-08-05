op babelforce-list-integration-providers -> Any
  description "List integration providers"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/providers")
  response = http.request(method: "GET", url)
  return response
