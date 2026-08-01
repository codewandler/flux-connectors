op babelforce-clear-setting(scope: String, key: String) -> Any
  description "Reset a setting"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/{scope}/{key}")
  response = http.request(method: "DELETE", url)
  return response
