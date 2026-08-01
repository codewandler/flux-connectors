op babelforce-get-widget-settings(type: String) -> Any
  description "Get UI feature flags and type-specific settings for a widget type"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/widget/{type}/settings")
  response = http.request(method: "GET", url)
  return response
