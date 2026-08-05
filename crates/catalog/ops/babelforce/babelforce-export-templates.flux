op babelforce-export-templates(type: String) -> Any
  description "Export configuration templates by type"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/templates/export/{type}")
  response = http.request(method: "GET", url)
  return response
