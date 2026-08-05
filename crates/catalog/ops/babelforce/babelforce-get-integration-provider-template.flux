op babelforce-get-integration-provider-template(provider: String) -> Any
  description "Get a provider config template"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{provider}/template")
  response = http.request(method: "GET", url)
  return response
