op babelforce-list-available-integrations -> Any
  description "List available integrations"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/available")
  response = http.request(method: "GET", url)
  return response
