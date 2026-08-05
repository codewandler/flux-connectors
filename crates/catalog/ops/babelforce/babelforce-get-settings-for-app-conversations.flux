op babelforce-get-settings-for-app-conversations -> Any
  description "Get conversations settings"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/app/conversations")
  response = http.request(method: "GET", url)
  return response
