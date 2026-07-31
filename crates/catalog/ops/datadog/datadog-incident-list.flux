op datadog-incident-list -> Any
  description "List the organization's incidents"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.datadoghq.com"
  url = fmt("{base}/api/v2/incidents")
  response = http.request(method: "GET", url)
  return response
