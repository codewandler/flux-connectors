op babelforce-list-provider-session-variables(provider: String) -> Any
  description "List a provider's session variables"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{provider}/actions/variables")
  response = http.request(method: "GET", url)
  return response
