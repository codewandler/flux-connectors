op babelforce-clear-all-settings -> Any
  description "Reset all settings"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings")
  response = http.request(method: "DELETE", url)
  return response
