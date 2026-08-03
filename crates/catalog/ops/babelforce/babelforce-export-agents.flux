op babelforce-export-agents(format: String) -> Any
  description "Export agents as CSV"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/provision")
  response = http.request(method: "GET", query: { format }, url)
  return response
