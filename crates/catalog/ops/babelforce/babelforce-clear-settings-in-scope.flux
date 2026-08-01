op babelforce-clear-settings-in-scope(scope: String) -> Any
  description "Reset a scope's settings"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/{scope}")
  response = http.request(method: "DELETE", url)
  return response
