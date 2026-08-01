op babelforce-get-settings-for-audit-default -> Any
  description "Get default settings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/audit/default")
  response = http.request(method: "GET", url)
  return response
