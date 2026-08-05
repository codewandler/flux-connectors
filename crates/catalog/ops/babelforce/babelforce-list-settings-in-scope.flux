op babelforce-list-settings-in-scope(scope: String) -> Any
  description "List a scope's settings"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/{scope}")
  response = http.request(method: "GET", url)
  return response
