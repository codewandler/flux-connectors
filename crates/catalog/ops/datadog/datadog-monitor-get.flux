op datadog-monitor-get(monitor_id: Number) -> Any
  description "Get one monitor by id"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.datadoghq.com"
  url = fmt("{base}/api/v1/monitor/{monitor_id}")
  response = http.request(method: "GET", url)
  return response
