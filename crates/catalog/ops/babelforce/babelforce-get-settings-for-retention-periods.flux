op babelforce-get-settings-for-retention-periods -> Any
  description "Get periods settings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/retention/periods")
  response = http.request(method: "GET", url)
  return response
