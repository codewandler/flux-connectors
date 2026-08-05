op babelforce-list-integration-type-actions(type: String) -> Any
  description "List an integration type's actions"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{type}/actions")
  response = http.request(method: "GET", url)
  return response
