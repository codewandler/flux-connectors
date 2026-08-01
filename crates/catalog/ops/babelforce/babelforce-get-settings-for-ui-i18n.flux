op babelforce-get-settings-for-ui-i18n -> Any
  description "Get i18n settings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/ui/i18n")
  response = http.request(method: "GET", url)
  return response
