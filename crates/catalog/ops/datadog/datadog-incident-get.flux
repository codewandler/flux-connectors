op datadog-incident-get(incident_id: String) -> Any
  description "Get one incident by id"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.datadoghq.com"
  url = fmt("{base}/api/v2/incidents/{incident_id}")
  response = http.request(method: "GET", url)
  return response
