op babelforce-list-all-settings -> Any
  description "List all settings"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings")
  response = http.request(method: "GET", url)
  return response
