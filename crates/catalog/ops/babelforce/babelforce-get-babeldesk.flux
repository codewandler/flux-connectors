op babelforce-get-babeldesk(id: String) -> Any
  description "Get Babeldesk"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/babeldesk/dashboards/{id}")
  response = http.request(method: "GET", url)
  return response
