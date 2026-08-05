op babelforce-get-babeldesk-widget(id: String) -> Any
  description "Get BabeldeskWidget"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/babeldesk/widgets/{id}")
  response = http.request(method: "GET", url)
  return response
