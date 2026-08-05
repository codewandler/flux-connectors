op babelforce-get-integration-template(type: String, provider: String) -> Any
  description "Get a type-scoped config template"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{type}/{provider}/template")
  response = http.request(method: "GET", url)
  return response
