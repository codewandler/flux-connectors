op babelforce-get-settings-for-app-integrations -> Any
  description "Get integrations settings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/app/integrations")
  response = http.request(method: "GET", url)
  return response
